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
use crate::audio::effects::AudioEffectsChain;
use crate::audio::mixer::resampling::RubatoSRC;
use crate::audio::{VUChannelService, VUProcessor};

/// Input processing worker for a specific device
pub struct InputWorker {
    pub device_id: String,
    pub device_sample_rate: u32, // Original device sample rate
    target_sample_rate: u32,     // Max sample rate for mixing (e.g., 48kHz)
    channels: u16,
    chunk_size: usize,   // Input device chunk size (for resampler)
    channel_number: u32,

    // Audio processing components
    resampler: Option<RubatoSRC>,
    effects_chain: AudioEffectsChain,

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
        chunk_size: usize, // Input device chunk size (e.g., from hardware buffer size)
        rtrb_consumer: rtrb::Consumer<f32>,
        input_notifier: Arc<Notify>,
        processed_output_tx: mpsc::UnboundedSender<ProcessedAudioSamples>,
        channel_number: u32,
    ) -> Self {
        info!("🎤 INPUT_WORKER: Creating RTRB-based worker for device '{}' ({} Hz → {} Hz, {} channels, channel #{})",
              device_id, device_sample_rate, target_sample_rate, channels, channel_number);

        Self {
            device_id,
            device_sample_rate,
            target_sample_rate,
            channels,
            chunk_size,
            channel_number,
            resampler: None,
            effects_chain: AudioEffectsChain::new(target_sample_rate),
            rtrb_consumer: Arc::new(Mutex::new(rtrb_consumer)),
            input_notifier,
            processed_output_tx,
            worker_handle: None,
            samples_processed: 0,
            processing_time_total: std::time::Duration::ZERO,
        }
    }

    /// Static helper function to get or initialize resampler in async context
    /// This can be used in the worker thread where we don't have access to &mut self
    fn get_or_initialize_resampler_static<'a>(
        resampler: &'a mut Option<RubatoSRC>,
        device_sample_rate: u32,
        target_sample_rate: u32,
        _chunk_size: usize, // Input device chunk size (unused with RubatoSRC)
        channels: u16,      // Channel count for resampler configuration
        device_id: &str,
    ) -> Option<&'a mut RubatoSRC> {
        let sample_rate_difference = (device_sample_rate as f32 - target_sample_rate as f32).abs();

        // No resampling needed if rates are close (within 1 Hz)
        if sample_rate_difference <= 1.0 {
            return None;
        }

        // Check if resampler exists and has the correct rates
        let needs_recreation = if let Some(ref existing_resampler) = resampler {
            existing_resampler.input_rate() != device_sample_rate
                || existing_resampler.output_rate() != target_sample_rate
        } else {
            true // No resampler exists
        };

        // Create or recreate resampler if needed
        if needs_recreation {
            // **MATCHING OUTPUT_WORKER**: Use same RubatoSRC configuration as output_worker
            // For input, we need to use sinc_fixed_output but we need the original channel count
            // The channel count should be determined dynamically based on the actual input device
            let frames_per_chunk = _chunk_size / channels as usize; // Convert samples to frames

            match RubatoSRC::new_fft_fixed_input(
                device_sample_rate as f32,
                target_sample_rate as f32,
                frames_per_chunk,  // Input frames from hardware buffer size
                channels as usize, // dynamic channel count
            ) {
                Ok(new_resampler) => {
                    info!(
                        "🔄 {}: {} resampler for {} ({} Hz → {} Hz, ratio: {:.3})",
                        "INPUT_RESAMPLER".green(),
                        if resampler.is_some() {
                            "Recreated"
                        } else {
                            "Created"
                        },
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

        // Return mutable reference to the resampler
        resampler.as_mut()
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
        let mut effects_chain = AudioEffectsChain::new(target_sample_rate);
        let mut vu_service_option: Option<Box<dyn VUProcessor>> = if let Some(channel) = vu_channel
        {
            info!(
                "🚀 INPUT_WORKER: Using VUChannelService for high-performance streaming ({})",
                device_id
            );
            Some(Box::new(VUChannelService::new(
                channel,
                target_sample_rate,
                8,
                60,
            ))) // 8 max channels, 60fps via channels
        } else {
            info!("⚠️ INPUT_WORKER: No VU service available ({})", device_id);
            None
        };

        // Spawn dedicated worker thread that waits for RTRB notifications
        let worker_handle = tokio::spawn(async move {
            let mut samples_processed = 0u64;

            info!(
                "🚀 INPUT_WORKER: Started RTRB notification-driven thread for device '{}'",
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
                let resampled_samples = if let Some(active_resampler) =
                    Self::get_or_initialize_resampler_static(
                        &mut resampler,
                        device_sample_rate,
                        target_sample_rate,
                        chunk_size,
                        channels, // Original input device channel count (before conversion)
                        &device_id,
                    ) {
                    // Resample using the active resampler (on original channel format)
                    active_resampler.convert(&samples)
                } else {
                    // No resampling needed or resampler creation failed
                    // WHy do we need to clone?
                    samples.clone()
                };
                // Step 2: Mono-to-stereo conversion (after resampling, before effects)
                let channel_converted_samples = if channels == 1 {
                    let converted = Self::convert_mono_to_stereo(&resampled_samples, &device_id);

                    converted
                } else {
                    resampled_samples
                };

                // Update channels to stereo for downstream processing (effects expect stereo)
                let processing_channels = if channels == 1 { 2 } else { channels };

                // Step 3: Apply per-input effects (EQ, compressor, etc.) - now on stereo data
                let mut effects_processed = channel_converted_samples;
                effects_chain.process(&mut effects_processed);

                // Step 3.5: Calculate and emit VU levels for this channel (if VU service available)
                if let Some(ref mut vu_service) = vu_service_option {
                    vu_service.process_channel_audio(channel_number, &effects_processed);
                }

                // Step 4: Send processed audio to mixing layer
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
                    warn!("⚠️ INPUT_WORKER: Failed to send processed audio for {} (mixing layer may be shut down)", device_id);
                    break;
                }

                // Performance tracking
                samples_processed += 1;
                let processing_duration = processing_start.elapsed();

                // Rate-limited logging
                if samples_processed <= 5 || samples_processed % 1000 == 0 {
                    info!(
                        "🔄 {}: (2nd layer) {} processed {} samples, sent {} in {}μs (batch #{})",
                        "RESAMPLE_AND_EFFECTS_INPUT_WORKER".green(),
                        device_id,
                        original_samples_len,
                        samples_to_send,
                        processing_duration.as_micros(),
                        samples_processed
                    );

                    // Log slow processing
                    if processing_duration.as_micros() > 500 {
                        warn!(
                            "⏱️ INPUT_WORKER: {} slow processing: {}μs",
                            device_id,
                            processing_duration.as_micros()
                        );
                    }
                }
            }

            info!(
                "🛑 INPUT_WORKER: Processing thread for '{}' shutting down (processed {} batches)",
                device_id, samples_processed
            );
        });

        self.worker_handle = Some(worker_handle);
        info!(
            "✅ INPUT_WORKER: Started worker thread for device '{}'",
            self.device_id
        );

        Ok(())
    }

    pub fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        info!(
            "🔄 {}: Updating target mix rate for '{}': {} Hz → {} Hz",
            "INPUT_WORKER_UPDATE".cyan(),
            self.device_id,
            self.target_sample_rate,
            target_mix_rate
        );

        self.target_sample_rate = target_mix_rate;
        self.update_effects(AudioEffectsChain::new(target_mix_rate));

        // **CRITICAL**: Force resampler recreation with new target rate
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

    /// Update effects chain for this input
    pub fn update_effects(&mut self, new_effects_chain: AudioEffectsChain) {
        self.effects_chain = new_effects_chain;
        info!(
            "🎛️ INPUT_WORKER: Updated effects chain for device '{}'",
            self.device_id
        );
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
