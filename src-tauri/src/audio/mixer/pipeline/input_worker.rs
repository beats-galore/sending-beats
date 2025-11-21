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

use super::audio_worker::{AudioWorker, AudioWorkerState};
use crate::audio::effects::{CustomAudioEffectsChain, DefaultAudioEffectsChain};
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::RubatoSRC;
use crate::audio::mixer::stream_management::virtual_mixer::VirtualMixer;
use crate::audio::VUChannelService;

/// Input processing worker for a specific device
pub struct InputWorker {
    state: AudioWorkerState,

    channel_number: u32,
    samples_processed: u64,
    processing_time_total: std::time::Duration,

    default_effects: Arc<Mutex<DefaultAudioEffectsChain>>,
    custom_effects: CustomAudioEffectsChain,
    any_channel_solo: Arc<std::sync::atomic::AtomicBool>,

    // Track hardware delivery cadence (before resampling) for effective rate calculation
    hardware_queue_tracker: AtomicQueueTracker,
}

impl InputWorker {
    pub fn new_with_rtrb(
        device_id: String,
        device_sample_rate: u32,
        target_sample_rate: u32,
        channels: u16,
        chunk_size: usize,
        rtrb_consumer: rtrb::Consumer<f32>,
        rtrb_producer: rtrb::Producer<f32>,
        channel_number: u32,
        any_channel_solo: Arc<std::sync::atomic::AtomicBool>,
        hardware_queue_tracker: AtomicQueueTracker,
        mixing_queue_tracker: AtomicQueueTracker,
        initial_gain: Option<f32>,
        initial_pan: Option<f32>,
        initial_muted: Option<bool>,
        initial_solo: Option<bool>,
    ) -> Self {
        info!(
            "🎤 {}: Creating worker for device '{}' ({} Hz → {} Hz, {} channels, channel #{})",
            "INPUT_WORKER".on_cyan().white(),
            device_id,
            device_sample_rate,
            target_sample_rate,
            channels,
            channel_number
        );

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

        let state = AudioWorkerState::new(
            device_id.clone(),
            device_sample_rate,
            target_sample_rate,
            channels,
            chunk_size,
            rtrb_consumer,
            rtrb_producer,
            mixing_queue_tracker,
        );

        Self {
            state,
            channel_number,
            default_effects: Arc::new(Mutex::new(default_effects)),
            custom_effects: CustomAudioEffectsChain::new(target_sample_rate),
            any_channel_solo,
            samples_processed: 0,
            processing_time_total: std::time::Duration::ZERO,
            hardware_queue_tracker,
        }
    }

    pub fn get_stats(&self) -> InputWorkerStats {
        InputWorkerStats {
            device_id: self.state.device_id().to_string(),
            device_sample_rate: self.state.device_sample_rate(),
            target_sample_rate: self.state.target_sample_rate(),
            samples_processed: self.samples_processed,
            is_running: true,
        }
    }

    pub fn channel_number(&self) -> u32 {
        self.channel_number
    }

    pub fn get_default_effects(&self) -> Arc<Mutex<DefaultAudioEffectsChain>> {
        self.default_effects.clone()
    }

    pub fn get_custom_effects_mut(&mut self) -> &mut CustomAudioEffectsChain {
        &mut self.custom_effects
    }
}

impl AudioWorker for InputWorker {
    fn device_id(&self) -> &str {
        self.state.device_id()
    }

    fn device_sample_rate(&self) -> u32 {
        self.state.device_sample_rate()
    }

    fn target_sample_rate(&self) -> u32 {
        self.state.target_sample_rate()
    }

    fn set_target_sample_rate(&mut self, rate: u32) {
        self.state.set_target_sample_rate(rate);
    }

    fn channels(&self) -> u16 {
        self.state.channels()
    }

    fn chunk_size(&self) -> usize {
        self.state.chunk_size()
    }

    fn set_chunk_size(&mut self, size: usize) {
        self.state.set_chunk_size(size);
    }

    fn resampler_mut(&mut self) -> &mut Option<RubatoSRC> {
        self.state.resampler_mut()
    }

    fn set_resampler(&mut self, resampler: Option<RubatoSRC>) {
        self.state.set_resampler(resampler);
    }

    fn queue_tracker(&self) -> &AtomicQueueTracker {
        self.state.queue_tracker()
    }

    fn rtrb_consumer(&self) -> &Arc<Mutex<rtrb::Consumer<f32>>> {
        self.state.rtrb_consumer()
    }

    fn rtrb_producer(&self) -> &Arc<Mutex<rtrb::Producer<f32>>> {
        self.state.rtrb_producer()
    }

    fn set_worker_handle(&mut self, handle: tokio::task::JoinHandle<()>) {
        self.state.set_worker_handle(handle);
    }

    fn take_worker_handle(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        self.state.take_worker_handle()
    }

    fn log_prefix(&self) -> &str {
        "INPUT_WORKER"
    }
}

impl InputWorker {
    pub fn start(
        &mut self,
        vu_channel: Option<tauri::ipc::Channel<crate::audio::VUChannelData>>,
    ) -> Result<()> {
        // Clone state for the post-processing closure
        let default_effects = self.default_effects.clone();
        let any_channel_solo = self.any_channel_solo.clone();
        let channel_number = self.channel_number;
        let channels = self.state.channels();

        // Clone hardware queue tracker for tracking hardware delivery cadence
        let hardware_tracker = self.hardware_queue_tracker.clone();

        let vu_service = vu_channel.map(|channel| {
            info!(
                "{}: VU channel enabled for {}",
                "VU_SETUP".on_cyan().white(),
                self.state.device_id()
            );
            VUChannelService::new(channel, self.state.target_sample_rate(), 8, 60)
        });

        // We need custom processing for InputWorker to track hardware cadence
        // Instead of using AudioWorker::start_processing_thread directly, implement here
        let device_id = self.state.device_id().to_string();
        let device_sample_rate = self.state.device_sample_rate();
        let target_sample_rate = self.state.target_sample_rate();
        let chunk_size = self.state.chunk_size();

        let rtrb_consumer = self.state.rtrb_consumer().clone();
        let rtrb_producer = self.state.rtrb_producer().clone();
        let queue_tracker = self.state.queue_tracker().clone();

        let mut resampler = self.state.resampler_mut().take();
        let mut input_accumulator = Vec::with_capacity(96000);

        info!(
            "🚀 {}: Starting processing thread with hardware cadence tracking for device '{}'",
            "INPUT_WORKER".on_cyan().white(),
            device_id
        );

        let worker_handle = tokio::spawn(async move {
            let mut samples_processed = 0u64;
            let mut samples_buffer = Vec::with_capacity(96000);

            loop {
                // Read samples from hardware RTRB
                samples_buffer.clear();
                let samples_read = {
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
                        continue;
                    }

                    let mut read_count = 0;
                    while read_count < available.min(96000) {
                        match consumer.pop() {
                            Ok(sample) => {
                                samples_buffer.push(sample);
                                read_count += 1;
                            }
                            Err(_) => break,
                        }
                    }
                    read_count
                };

                if samples_read == 0 {
                    continue;
                }

                // IMPORTANT: Track hardware delivery cadence HERE (before resampling)
                hardware_tracker.record_samples_written(samples_read);

                // Pre-accumulate incoming samples
                input_accumulator.extend_from_slice(&samples_buffer[..samples_read]);

                // Process all available chunks from the accumulator
                let mut total_samples_written_this_iteration = 0;
                loop {
                    let accumulated_samples =
                        <InputWorker as AudioWorker>::process_with_pre_accumulation(
                            &mut resampler,
                            &mut input_accumulator,
                            chunk_size,
                            device_id.clone(),
                        );

                    let accumulated_samples = match accumulated_samples {
                        Some(samples) => samples,
                        None => break,
                    };

                    let processing_start = std::time::Instant::now();

                    // Check for effective sample rate from HARDWARE cadence measurements
                    let effective_rate = hardware_tracker.get_effective_sample_rate();
                    let input_rate = effective_rate.unwrap_or(device_sample_rate);

                    // Log when we detect a mismatch
                    if let Some(eff_rate) = effective_rate {
                        let rate_diff = (eff_rate as f32 - device_sample_rate as f32).abs();
                        if rate_diff > 100.0 {
                            static MISMATCH_LOG: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let log_count =
                                MISMATCH_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if log_count < 20 || log_count % 500 == 0 {
                                info!(
                                    "⚠️ {}: Device '{}' rate mismatch - nominal: {} Hz, measured: {} Hz (diff: {:.0} Hz)",
                                    "RATE_MISMATCH".on_red().white(),
                                    device_id,
                                    device_sample_rate,
                                    eff_rate,
                                    rate_diff
                                );
                            }
                        }
                    }

                    // Always resample to handle clock drift and sample rate mismatches
                    let resample_start = std::time::Instant::now();
                    let processed_samples = if let Some(active_resampler) =
                        <InputWorker as AudioWorker>::get_or_initialize_resampler_static(
                            &mut resampler,
                            input_rate,
                            target_sample_rate,
                            chunk_size,
                            channels,
                            &device_id,
                        ) {
                        let resampled = active_resampler.convert(&accumulated_samples);

                        // Apply dynamic rate adjustment for fine drift correction
                        let _ = <InputWorker as AudioWorker>::adjust_dynamic_sample_rate(
                            active_resampler,
                            &queue_tracker,
                            input_rate,
                            target_sample_rate,
                            &device_id,
                        );

                        static RESAMPLE_LOG_COUNT: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let resample_count =
                            RESAMPLE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        if resample_count < 10 || resample_count % 1000 == 0 {
                            info!(
                                "🔄 {}: Resampled {} input → {} output ({}Hz→{}Hz) {}",
                                "AUDIO_RESAMPLE".on_cyan().white(),
                                accumulated_samples.len(),
                                resampled.len(),
                                input_rate,
                                target_sample_rate,
                                device_id
                            );
                        }

                        resampled
                    } else {
                        warn!(
                            "⚠️ INPUT_WORKER[{}]: Failed to initialize resampler, passing through",
                            device_id
                        );
                        accumulated_samples
                    };
                    let resample_duration = resample_start.elapsed();

                    // Apply post-processing (effects, VU meters, etc.)
                    let post_process_start = std::time::Instant::now();
                    let mut final_samples = processed_samples;

                    // Inline post-processing logic
                    {
                        let samples = &mut final_samples;
                        let device_id_ref = device_id.as_str();

                        // Mono-to-stereo conversion
                        if channels == 1 {
                            let original_count = samples.len();
                            *samples = VirtualMixer::convert_mono_to_stereo(samples);
                            let converted_count = samples.len();

                            static MONO_STEREO_LOG_COUNT: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let log_count = MONO_STEREO_LOG_COUNT
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if log_count < 20 || log_count % 500 == 0 {
                                info!(
                                    "🔄 {}: Device '{}' mono→stereo: {} → {} samples ({}x)",
                                    "MONO_TO_STEREO".yellow(),
                                    device_id_ref,
                                    original_count,
                                    converted_count,
                                    converted_count as f32 / original_count as f32
                                );
                            }
                        }

                        // Apply default effects
                        let any_solo = any_channel_solo.load(std::sync::atomic::Ordering::Relaxed);
                        if let Ok(effects) = default_effects.lock() {
                            effects.process_stereo_interleaved(samples, any_solo);
                        }

                        // VU metering
                        if let Some(ref vu) = vu_service {
                            vu.queue_channel_audio(channel_number, samples);
                        }
                    }

                    let post_process_duration = post_process_start.elapsed();

                    // Write to output RTRB queue
                    let write_start = std::time::Instant::now();
                    let samples_written = <InputWorker as AudioWorker>::write_samples_to_rtrb_sync(
                        &device_id,
                        &final_samples,
                        &rtrb_producer,
                    );
                    total_samples_written_this_iteration += samples_written;
                    let write_duration = write_start.elapsed();

                    samples_processed += 1;
                    let processing_duration = processing_start.elapsed();

                    if samples_processed <= 5 || samples_processed % 1000 == 0 {
                        info!(
                            "🔄 {}: {} processed {} samples in {}μs (resample: {}μs, post: {}μs, write: {}μs) batch #{}",
                            "INPUT_WORKER".on_cyan().white(),
                            device_id,
                            final_samples.len(),
                            processing_duration.as_micros(),
                            resample_duration.as_micros(),
                            post_process_duration.as_micros(),
                            write_duration.as_micros(),
                            samples_processed
                        );
                    }

                    if processing_duration.as_micros() > 500 {
                        warn!(
                            "⏱️ {}: {} SLOW processing: {}μs total (resample: {}μs, post: {}μs, write: {}μs)",
                            "INPUT_WORKER".on_cyan().white(),
                            device_id,
                            processing_duration.as_micros(),
                            resample_duration.as_micros(),
                            post_process_duration.as_micros(),
                            write_duration.as_micros()
                        );
                    }
                }

                // Track output cadence for mixing layer
                if total_samples_written_this_iteration > 0 {
                    queue_tracker.record_samples_written(total_samples_written_this_iteration);
                }
            }
        });

        self.state.set_worker_handle(worker_handle);
        info!(
            "✅ {}: Started worker thread with hardware cadence tracking for device '{}'",
            "INPUT_WORKER".on_cyan().white(),
            self.state.device_id()
        );

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        AudioWorker::stop(self).await
    }

    pub fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        self.update_custom_effects(CustomAudioEffectsChain::new(target_mix_rate));
        AudioWorker::update_target_mix_rate(self, target_mix_rate)
    }

    pub fn update_custom_effects(&mut self, new_effects_chain: CustomAudioEffectsChain) {
        self.custom_effects = new_effects_chain;
        info!(
            "🎛️ {}: Updated custom effects chain for device '{}'",
            "INPUT_WORKER".on_cyan().white(),
            self.state.device_id()
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
    pub fn get_queue_tracker_for_consumer(&self) -> AtomicQueueTracker {
        self.queue_tracker().clone()
    }
}

#[derive(Debug, Clone)]
pub struct InputWorkerStats {
    pub device_id: String,
    pub device_sample_rate: u32,
    pub target_sample_rate: u32,
    pub samples_processed: u64,
    pub is_running: bool,
}
