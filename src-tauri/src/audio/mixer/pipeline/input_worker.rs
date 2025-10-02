// Layer 2: Input Processing Workers
//
// Each input device gets its own dedicated worker thread that:
// 1. Receives raw audio from Layer 1 input capture
// 2. Resamples to maximum sample rate (e.g., 48kHz)
// 3. Applies per-input effects (EQ, compressor, etc.)
// 4. Sends processed audio to Layer 3 mixing

use anyhow::Result;
use colored::*;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Notify};
use tracing::{error, info, warn};

use super::queue_types::ProcessedAudioSamples;
use crate::audio::effects::{CustomAudioEffectsChain, DefaultAudioEffectsChain};
use crate::audio::mixer::pipeline::resampling_accumulator;
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::RubatoSRC;
use crate::audio::VUChannelService;

/// Input processing worker for a specific device
pub struct InputWorker {
    pub device_id: String,
    pub device_sample_rate: u32, // Original device sample rate
    target_sample_rate: u32,     // Max sample rate for mixing (e.g., 48kHz)
    channels: u16,
    chunk_size: usize, // Input device chunk size (for resampler)
    channel_number: u32,

    // Audio processing components
    resampler: Option<RubatoSRC>,
    default_effects: Arc<Mutex<DefaultAudioEffectsChain>>,
    custom_effects: CustomAudioEffectsChain,
    any_channel_solo: Arc<std::sync::atomic::AtomicBool>,
    queue_tracker: AtomicQueueTracker, // Track output queue for dynamic resampling

    // **DIRECT RTRB**: Read directly from hardware RTRB queue
    rtrb_consumer: Arc<Mutex<rtrb::Consumer<f32>>>,
    input_notifier: Arc<Notify>, // Our own notification for input data available

    // Output to mixing layer
    processed_output_tx: mpsc::UnboundedSender<ProcessedAudioSamples>,

    // Worker thread handle
    worker_handle: Option<tokio::task::JoinHandle<()>>,

    // Performance metrics
    samples_processed: u64,
    processing_time_total: std::time::Duration,
}

impl InputWorker {
    /// Convert mono audio samples to stereo by duplicating each sample to both channels
    fn convert_mono_to_stereo(mono_samples: &[f32], _device_id: &str) -> Vec<f32> {
        let mut stereo_samples = Vec::with_capacity(mono_samples.len() * 2);
        for &mono_sample in mono_samples {
            stereo_samples.push(mono_sample); // Left channel
            stereo_samples.push(mono_sample); // Right channel (duplicate)
        }
        stereo_samples
    }

    /// Create a new input processing worker that reads directly from RTRB
    pub fn new_with_rtrb(
        device_id: String,
        device_sample_rate: u32,
        target_sample_rate: u32,
        channels: u16,
        chunk_size: usize,
        rtrb_consumer: rtrb::Consumer<f32>,
        input_notifier: Arc<Notify>,
        processed_output_tx: mpsc::UnboundedSender<ProcessedAudioSamples>,
        channel_number: u32,
        any_channel_solo: Arc<std::sync::atomic::AtomicBool>,
        queue_tracker: AtomicQueueTracker,
        initial_gain: Option<f32>,
        initial_pan: Option<f32>,
        initial_muted: Option<bool>,
        initial_solo: Option<bool>,
    ) -> Self {
        info!("🎤 {}: Creating RTRB-based worker for device '{}' ({} Hz → {} Hz, {} channels, channel #{})",
        "INPUT_WORKER".on_cyan().white(),
              device_id, device_sample_rate, target_sample_rate, channels, channel_number);

        let mut default_effects = DefaultAudioEffectsChain::new(device_id.clone());

        if let Some(gain) = initial_gain {
            default_effects.set_gain(gain);
            info!(
                "🔊 {}: Initialized gain for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                gain
            );
        }
        if let Some(pan) = initial_pan {
            default_effects.set_pan(pan);
            info!(
                "🎚️ {}: Initialized pan for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                pan
            );
        }
        if let Some(muted) = initial_muted {
            default_effects.set_muted(muted);
            info!(
                "🔇 {}: Initialized muted for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                muted
            );
        }
        if let Some(solo) = initial_solo {
            default_effects.set_solo(solo);
            info!(
                "🎯 {}: Initialized solo for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                solo
            );
        }

        Self {
            device_id: device_id.clone(),
            device_sample_rate,
            target_sample_rate,
            channels,
            chunk_size,
            channel_number,
            resampler: None,
            default_effects: Arc::new(Mutex::new(default_effects)),
            custom_effects: CustomAudioEffectsChain::new(target_sample_rate),
            any_channel_solo,
            queue_tracker,
            rtrb_consumer: Arc::new(Mutex::new(rtrb_consumer)),
            input_notifier,
            processed_output_tx,
            worker_handle: None,
            samples_processed: 0,
            processing_time_total: std::time::Duration::ZERO,
        }
    }

    /// Static helper function to get or initialize resampler in async context
    /// Since we use SincFixedOut, we can dynamically adjust rates without recreation
    fn get_or_initialize_resampler_static<'a>(
        resampler: &'a mut Option<RubatoSRC>,
        device_sample_rate: u32,
        target_sample_rate: u32,
        _chunk_size: usize, // Input device chunk size
        channels: u16,      // Channel count for resampler configuration
        device_id: &str,
    ) -> Option<&'a mut RubatoSRC> {
        let sample_rate_difference = (device_sample_rate as f32 - target_sample_rate as f32).abs();

        // No resampling needed if rates are close (within 1 Hz)
        if sample_rate_difference <= 1.0 {
            return None;
        }

        // If resampler exists, adjust it dynamically; otherwise create new one
        match resampler {
            Some(ref mut existing_resampler) => {
                let rates_changed = existing_resampler.input_rate() != device_sample_rate
                    || existing_resampler.output_rate() != target_sample_rate;

                if rates_changed {
                    // Use dynamic adjustment - SincFixedOut supports this
                    if let Err(e) = existing_resampler.set_sample_rates(
                        device_sample_rate as f32,
                        target_sample_rate as f32,
                        true, // Use ramping for smooth transitions
                    ) {
                        warn!(
                            "⚠️ {}: Dynamic adjustment failed for {}: {} - recreating resampler",
                            "DYNAMIC_ADJUST_FAILED".on_cyan().white(),
                            device_id,
                            e
                        );
                        // If dynamic adjustment fails, create a new resampler
                        *resampler = None;
                    }
                }
            }
            None => {
                // Create new resampler
                let frames_per_chunk = _chunk_size / channels as usize; // Convert samples to frames

                match RubatoSRC::new_sinc_fixed_output(
                    device_sample_rate as f32,
                    target_sample_rate as f32,
                    frames_per_chunk,  // Output frames we want to produce
                    channels as usize, // dynamic channel count
                    format!("input_{}", device_id), // Identifier for logging
                ) {
                    Ok(new_resampler) => {
                        info!(
                            "🔄 {}: Created resampler for {} ({} Hz → {} Hz, ratio: {:.3})",
                            "INPUT_RESAMPLER".on_cyan().white(),
                            device_id,
                            device_sample_rate,
                            target_sample_rate,
                            new_resampler.ratio()
                        );
                        *resampler = Some(new_resampler);
                    }
                    Err(e) => {
                        error!(
                            "❌ INPUT_WORKER: Failed to create resampler for {}: {}",
                            device_id, e
                        );
                        return None;
                    }
                }
            }
        }

        // Return mutable reference to the resampler
        resampler.as_mut()
    }

    /// Get queue tracker for sharing with mixing layer (consumer side)
    pub fn get_queue_tracker_for_consumer(&self) -> AtomicQueueTracker {
        self.queue_tracker.clone()
    }

    /// Start the input processing worker thread
    pub fn start(
        &mut self,
        vu_channel: Option<tauri::ipc::Channel<crate::audio::VUChannelData>>,
    ) -> Result<()> {
        let device_id = self.device_id.clone();
        let device_sample_rate = self.device_sample_rate;
        let target_sample_rate = self.target_sample_rate;
        let channels = self.channels;
        let chunk_size = self.chunk_size;
        let channel_number = self.channel_number;

        // Clone shared resources for the worker thread
        let rtrb_consumer = self.rtrb_consumer.clone();
        let input_notifier = self.input_notifier.clone();
        let processed_output_tx = self.processed_output_tx.clone();

        let mut resampler = self.resampler.take();
        let mut input_accumulator = Vec::with_capacity(8192); // Accumulator for pre-resampling
        let queue_tracker = self.queue_tracker.clone(); // For dynamic rate adjustment (shared with mixing layer)
        let default_effects = self.default_effects.clone(); // Arc clone - shares the same data
        let mut custom_effects = CustomAudioEffectsChain::new(target_sample_rate);
        let any_channel_solo = self.any_channel_solo.clone();
        let vu_service = vu_channel.map(|channel| {
            info!(
                "{}: VU channel enabled for {}",
                "VU_SETUP".on_cyan().white(),
                device_id
            );
            VUChannelService::new(channel, target_sample_rate, 8, 60)
        });

        // Spawn dedicated worker thread that waits for RTRB notifications
        let worker_handle = tokio::spawn(async move {
            let mut samples_processed = 0u64;

            info!(
                "🚀 {}: Started RTRB notification-driven thread for device '{}'",
                "INPUT_WORKER".on_cyan().white(),
                device_id
            );

            loop {
                input_notifier.notified().await;

                // Read available samples from RTRB
                let samples = {
                    let mut consumer = match rtrb_consumer.try_lock() {
                        Ok(consumer) => consumer,
                        Err(_) => {
                            warn!(
                                "⚠️ INPUT_WORKER[{}]: Failed to lock RTRB consumer",
                                device_id
                            );
                            continue;
                        }
                    };

                    let available = consumer.slots();
                    if available == 0 {
                        continue; // No data available
                    }

                    let mut samples = Vec::with_capacity(available.min(8192)); // Limit to reasonable chunk size
                    let mut read_count = 0;
                    while read_count < available.min(8192) {
                        match consumer.pop() {
                            Ok(sample) => {
                                samples.push(sample);
                                read_count += 1;
                            }
                            Err(_) => break, // No more samples available
                        }
                    }
                    samples
                };

                if samples.is_empty() {
                    continue;
                }
                let processing_start = std::time::Instant::now();
                let original_samples_len = samples.len();

                // Step 1: Resample to target sample rate if needed (more efficient with original mono data)
                let resample_start = std::time::Instant::now();
                let resampled_samples = if let Some(active_resampler) =
                    Self::get_or_initialize_resampler_static(
                        &mut resampler,
                        device_sample_rate,
                        target_sample_rate,
                        chunk_size,
                        channels, // Original input device channel count (before conversion)
                        &device_id,
                    ) {
                    // Use pre-accumulation for upsampling: collect input until we have enough

                    if let Some(resampled) = resampling_accumulator::process_with_pre_accumulation(
                        active_resampler,
                        &samples,
                        &mut input_accumulator,
                        chunk_size, // Target output sample count
                    ) {
                        // Apply dynamic rate adjustment after successful resampling
                        let _ = resampling_accumulator::adjust_dynamic_sample_rate(
                            active_resampler,
                            &queue_tracker,
                            device_sample_rate,
                            target_sample_rate,
                            &device_id,
                        );

                        // Log resampling activity
                        static RESAMPLE_LOG_COUNT: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let resample_count =
                            RESAMPLE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        if resample_count < 10 || resample_count % 100 == 0 {
                            info!(
                                "🔄 {}: Resampled {} input → {} output (buffer: {} samples, {}Hz→{}Hz)",
                                "INPUT_RESAMPLE".on_cyan().white(),
                                samples.len(),
                                resampled.len(),
                                input_accumulator.len(),
                                device_sample_rate,
                                target_sample_rate
                            );
                        }

                        resampled
                    } else {
                        // Not enough accumulated yet, skip this iteration
                        static SKIP_LOG_COUNT: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let skip_count =
                            SKIP_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        if skip_count < 10 || skip_count % 500 == 0 {
                            info!(
                                "⏸️ {}: Accumulating samples (buffer: {} samples, need more for output)",
                                "INPUT_ACCUM_SKIP".on_cyan().white(),
                                input_accumulator.len()
                            );
                        }

                        continue;
                    }
                } else {
                    // No resampling needed or resampler creation failed
                    samples.clone()
                };
                let resample_duration = resample_start.elapsed();

                // Step 2: Mono-to-stereo conversion (after resampling, before effects)
                let conversion_start = std::time::Instant::now();
                let channel_converted_samples = if channels == 1 {
                    let converted = Self::convert_mono_to_stereo(&resampled_samples, &device_id);

                    converted
                } else {
                    resampled_samples
                };
                let conversion_duration = conversion_start.elapsed();

                // Update channels to stereo for downstream processing (effects expect stereo)
                let processing_channels = if channels == 1 { 2 } else { channels };

                // Step 3: Apply default effects (gain/pan/mute/solo) BEFORE custom effects
                let effects_start = std::time::Instant::now();
                let mut effects_processed = channel_converted_samples;

                let any_solo = any_channel_solo.load(std::sync::atomic::Ordering::Relaxed);
                if let Ok(effects) = default_effects.lock() {
                    if processing_channels == 2 {
                        effects.process_stereo_interleaved(&mut effects_processed, any_solo);
                    } else {
                        effects.process_mono(&mut effects_processed, any_solo);
                    }
                } else {
                    warn!(
                        "⚠️ {}[{}]: Failed to lock default effects",
                        "INPUT_WORKER".on_cyan().white(),
                        device_id
                    );
                }

                custom_effects.process(&mut effects_processed);
                let effects_duration = effects_start.elapsed();

                // Step 3.5: Calculate and emit VU levels for this channel (if VU service available)
                let vu_start = std::time::Instant::now();
                if let Some(ref vu_service) = vu_service {
                    vu_service.queue_channel_audio(channel_number, &effects_processed);
                }
                let vu_duration = vu_start.elapsed();

                // Step 4: Send processed audio to mixing layer
                let send_start = std::time::Instant::now();
                let samples_to_send = effects_processed.len();
                let processed_audio = ProcessedAudioSamples {
                    device_id: device_id.clone(),
                    samples: effects_processed,
                    channels: processing_channels, // Use converted channel count (stereo)
                    timestamp: std::time::Instant::now(),
                    effects_applied: true,
                };

                // Send to Layer 3 mixing
                if let Err(_) = processed_output_tx.send(processed_audio) {
                    warn!("⚠️ {}: Failed to send processed audio for {} (mixing layer may be shut down)", "INPUT_WORKER".on_cyan().white(), device_id);
                    break;
                }

                // Record samples written for queue tracking (producer side)
                queue_tracker.record_samples_written(samples_to_send);

                let send_duration = send_start.elapsed();

                // Performance tracking
                samples_processed += 1;
                let processing_duration = processing_start.elapsed();

                // Rate-limited logging with detailed breakdown
                if samples_processed <= 5 || samples_processed % 1000 == 0 {
                    info!(
                        "🔄 {}: {} processed {} samples in {}μs (resample: {}μs, conv: {}μs, effects: {}μs, vu: {}μs, send: {}μs) batch #{}",
                        "INPUT_WORKER_TIMING".on_cyan().white(),
                        device_id,
                        original_samples_len,
                        processing_duration.as_micros(),
                        resample_duration.as_micros(),
                        conversion_duration.as_micros(),
                        effects_duration.as_micros(),
                        vu_duration.as_micros(),
                        send_duration.as_micros(),
                        samples_processed
                    );
                }

                // Log slow processing with breakdown
                if processing_duration.as_micros() > 500 {
                    warn!(
                        "⏱️ {}: {} SLOW processing: {}μs total (resample: {}μs, conv: {}μs, effects: {}μs, vu: {}μs, send: {}μs)",
                        "INPUT_WORKER_SLOW".on_cyan().white(),
                        device_id,
                        processing_duration.as_micros(),
                        resample_duration.as_micros(),
                        conversion_duration.as_micros(),
                        effects_duration.as_micros(),
                        vu_duration.as_micros(),
                        send_duration.as_micros()
                    );
                }
            }

            info!(
                "🛑 {}: Processing thread for '{}' shutting down (processed {} batches)",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                samples_processed
            );
        });

        self.worker_handle = Some(worker_handle);
        info!(
            "✅ {}: Started worker thread for device '{}'",
            "INPUT_WORKER".on_cyan().white(),
            self.device_id
        );

        Ok(())
    }

    pub fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        info!(
            "🔄 {}: Updating target mix rate for '{}': {} Hz → {} Hz",
            "INPUT_WORKER_UPDATE".on_cyan().white(),
            self.device_id,
            self.target_sample_rate,
            target_mix_rate
        );

        self.target_sample_rate = target_mix_rate;
        self.update_custom_effects(CustomAudioEffectsChain::new(target_mix_rate));

        self.resampler = None;

        Ok(())
    }

    /// Stop the input processing worker
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();

            // Wait for graceful shutdown
            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => info!("✅ INPUT_WORKER: '{}' shut down gracefully", self.device_id),
                Err(_) => warn!(
                    "⚠️ INPUT_WORKER: '{}' force-terminated after timeout",
                    self.device_id
                ),
            }
        }

        Ok(())
    }

    pub fn update_custom_effects(&mut self, new_effects_chain: CustomAudioEffectsChain) {
        self.custom_effects = new_effects_chain;
        info!(
            "🎛️ {}: Updated custom effects chain for device '{}'",
            "INPUT_WORKER".on_cyan().white(),
            self.device_id
        );
    }

    pub fn update_gain(&mut self, gain: f32) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_gain(gain);
        }
    }

    pub fn update_pan(&mut self, pan: f32) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_pan(pan);
        }
    }

    pub fn update_muted(&mut self, muted: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_muted(muted);
        }
    }

    pub fn update_solo(&mut self, solo: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_solo(solo);
        }
        self.any_channel_solo
            .store(solo, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get processing statistics
    pub fn get_stats(&self) -> InputWorkerStats {
        InputWorkerStats {
            device_id: self.device_id.clone(),
            samples_processed: self.samples_processed,
            average_processing_time: if self.samples_processed > 0 {
                self.processing_time_total / self.samples_processed as u32
            } else {
                std::time::Duration::ZERO
            },
            is_running: self.worker_handle.is_some(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputWorkerStats {
    pub device_id: String,
    pub samples_processed: u64,
    pub average_processing_time: std::time::Duration,
    pub is_running: bool,
}
