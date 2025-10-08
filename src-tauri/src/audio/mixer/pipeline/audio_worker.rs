// Generic Audio Worker Trait
//
// Shared trait for both input and output workers that provides:
// 1. Shared start() method with common processing loop
// 2. Resampler initialization and dynamic rate adjustment
// 3. Adaptive chunk sizing
// 4. Customizable post-processing hook
//
// Input workers implement apply_post_processing() with effects/VU logic
// Output workers implement it as a no-op

use anyhow::Result;
use colored::*;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::RubatoSRC;
/// Shared state for audio workers
pub struct AudioWorkerState {
    pub device_id: String,
    pub device_sample_rate: u32,
    pub target_sample_rate: u32,
    pub channels: u16,
    pub chunk_size: usize,
    pub resampler: Option<RubatoSRC>,
    pub queue_tracker: AtomicQueueTracker,
    pub rtrb_consumer: Arc<Mutex<rtrb::Consumer<f32>>>,
    pub rtrb_producer: Arc<Mutex<rtrb::Producer<f32>>>,
    pub worker_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AudioWorkerState {
    pub fn new(
        device_id: String,
        device_sample_rate: u32,
        target_sample_rate: u32,
        channels: u16,
        chunk_size: usize,
        rtrb_consumer: rtrb::Consumer<f32>,
        rtrb_producer: rtrb::Producer<f32>,
        queue_tracker: AtomicQueueTracker,
    ) -> Self {
        Self {
            device_id,
            device_sample_rate,
            target_sample_rate,
            channels,
            chunk_size,
            resampler: None,
            queue_tracker,
            rtrb_consumer: Arc::new(Mutex::new(rtrb_consumer)),
            rtrb_producer: Arc::new(Mutex::new(rtrb_producer)),
            worker_handle: None,
        }
    }

    // Getters
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn device_sample_rate(&self) -> u32 {
        self.device_sample_rate
    }

    pub fn target_sample_rate(&self) -> u32 {
        self.target_sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn resampler_mut(&mut self) -> &mut Option<RubatoSRC> {
        &mut self.resampler
    }

    pub fn queue_tracker(&self) -> &AtomicQueueTracker {
        &self.queue_tracker
    }

    pub fn rtrb_consumer(&self) -> &Arc<Mutex<rtrb::Consumer<f32>>> {
        &self.rtrb_consumer
    }

    pub fn rtrb_producer(&self) -> &Arc<Mutex<rtrb::Producer<f32>>> {
        &self.rtrb_producer
    }

    pub fn worker_handle(&self) -> &Option<tokio::task::JoinHandle<()>> {
        &self.worker_handle
    }

    // Setters
    pub fn set_target_sample_rate(&mut self, rate: u32) {
        self.target_sample_rate = rate;
    }

    pub fn set_chunk_size(&mut self, size: usize) {
        self.chunk_size = size;
    }

    pub fn set_resampler(&mut self, resampler: Option<RubatoSRC>) {
        self.resampler = resampler;
    }

    pub fn set_worker_handle(&mut self, handle: tokio::task::JoinHandle<()>) {
        self.worker_handle = Some(handle);
    }

    pub fn take_worker_handle(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        self.worker_handle.take()
    }
}

/// Trait providing shared functionality for audio workers
pub trait AudioWorker {
    /// Get device ID
    fn device_id(&self) -> &str;

    /// Get device sample rate
    fn device_sample_rate(&self) -> u32;

    /// Get target sample rate
    fn target_sample_rate(&self) -> u32;

    /// Set target sample rate
    fn set_target_sample_rate(&mut self, rate: u32);

    /// Get channels
    fn channels(&self) -> u16;

    /// Get chunk size
    fn chunk_size(&self) -> usize;

    /// Set chunk size
    fn set_chunk_size(&mut self, size: usize);

    /// Get mutable resampler reference
    fn resampler_mut(&mut self) -> &mut Option<RubatoSRC>;

    /// Set resampler
    fn set_resampler(&mut self, resampler: Option<RubatoSRC>);

    /// Get queue tracker
    fn queue_tracker(&self) -> &AtomicQueueTracker;

    /// Get RTRB consumer (for reading samples)
    fn rtrb_consumer(&self) -> &Arc<Mutex<rtrb::Consumer<f32>>>;

    /// Get RTRB producer (for writing samples)
    fn rtrb_producer(&self) -> &Arc<Mutex<rtrb::Producer<f32>>>;

    /// Set worker handle
    fn set_worker_handle(&mut self, handle: tokio::task::JoinHandle<()>);

    /// Take worker handle
    fn take_worker_handle(&mut self) -> Option<tokio::task::JoinHandle<()>>;

    /// Get log prefix for this worker type (e.g., "INPUT_WORKER", "OUTPUT_WORKER")
    fn log_prefix(&self) -> &str;

    /// Get or initialize resampler with dynamic rate adjustment support
    fn get_or_initialize_resampler_static<'a>(
        resampler: &'a mut Option<RubatoSRC>,
        device_sample_rate: u32,
        target_sample_rate: u32,
        chunk_size: usize,
        channels: u16,
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
                    if let Err(e) = existing_resampler.set_sample_rates(
                        device_sample_rate as f32,
                        target_sample_rate as f32,
                        true,
                        device_id.to_string(),
                    ) {
                        warn!(
                            "⚠️ {}: Dynamic adjustment failed for {}: {} - recreating resampler",
                            "DYNAMIC_ADJUST_FAILED".on_cyan().white(),
                            device_id,
                            e
                        );
                        *resampler = None;
                    }
                }
            }
            None => {
                let frames_per_chunk = chunk_size / channels as usize;

                match RubatoSRC::new_sinc_fixed_output(
                    device_sample_rate as f32,
                    target_sample_rate as f32,
                    frames_per_chunk,
                    channels as usize,
                    format!("worker_{}", device_id),
                ) {
                    Ok(new_resampler) => {
                        info!(
                            "🔄 {}: Created resampler for {} ({} Hz → {} Hz, ratio: {:.3})",
                            "RESAMPLER_INIT".on_cyan().white(),
                            device_id,
                            device_sample_rate,
                            target_sample_rate,
                            new_resampler.ratio()
                        );
                        *resampler = Some(new_resampler);
                    }
                    Err(e) => {
                        warn!(
                            "❌ {}: Failed to create resampler for {}: {}",
                            "AUDIO_WORKER".on_cyan().white(),
                            device_id,
                            e
                        );
                        return None;
                    }
                }
            }
        }

        resampler.as_mut()
    }

    /// Write samples to RTRB queue synchronously
    fn write_samples_to_rtrb_sync(
        device_id: &str,
        samples: &[f32],
        rtrb_producer: &Arc<Mutex<rtrb::Producer<f32>>>,
        queue_tracker: Option<&AtomicQueueTracker>,
    ) {
        if let Ok(mut producer) = rtrb_producer.try_lock() {
            let mut samples_written = 0;
            let mut remaining = samples;

            while !remaining.is_empty() && samples_written < samples.len() {
                let chunk_size = remaining.len().min(producer.slots());
                if chunk_size == 0 {
                    // warn!(
                    //     "⚠️ {}: {} RTRB queue full, dropping {} remaining samples",
                    //     "AUDIO_WORKER".on_cyan().white(),
                    //     device_id,
                    //     remaining.len()
                    // );
                    break;
                }

                let chunk = &remaining[..chunk_size];
                for &sample in chunk {
                    if producer.push(sample).is_err() {
                        break;
                    }
                    samples_written += 1;
                }
                remaining = &remaining[chunk_size..];
            }

            if let Some(tracker) = queue_tracker {
                tracker.record_samples_written(samples_written);
            }
        } else {
            warn!(
                "⚠️ AUDIO_WORKER: {} failed to lock RTRB producer, dropping {} samples",
                device_id,
                samples.len()
            );
        }
    }

    /// Shared start method - implements the common processing loop
    /// Takes an optional post-processing function that will be called on resampled audio
    fn start_processing_thread<F>(&mut self, mut post_process_fn: Option<F>) -> Result<()>
    where
        F: FnMut(&mut Vec<f32>, u16, &str) -> Result<()> + Send + 'static,
    {
        let device_id = self.device_id().to_string();
        let device_sample_rate = self.device_sample_rate();
        let initial_target_sample_rate = self.target_sample_rate();
        let channels = self.channels();
        let chunk_size = self.chunk_size();
        let log_prefix = self.log_prefix().to_string();

        // Clone shared resources for the worker thread
        let rtrb_consumer = self.rtrb_consumer().clone();
        let rtrb_producer = self.rtrb_producer().clone();
        let queue_tracker = self.queue_tracker().clone();

        let mut resampler = self.resampler_mut().take();
        let mut input_accumulator = Vec::with_capacity(96000);

        info!(
            "🚀 {}: Starting processing thread for device '{}'",
            log_prefix.on_cyan().white(),
            device_id
        );

        // Spawn dedicated worker thread
        let worker_handle = tokio::spawn(async move {
            let mut samples_processed = 0u64;
            let mut samples_buffer = Vec::with_capacity(96000);

            info!(
                "🚀 {}: Started processing thread for device '{}'",
                log_prefix.on_cyan().white(),
                device_id
            );

            loop {
                // Read available samples from RTRB consumer
                samples_buffer.clear();
                let samples = {
                    let mut consumer = match rtrb_consumer.try_lock() {
                        Ok(consumer) => consumer,
                        Err(_) => {
                            warn!(
                                "⚠️ {}[{}]: Failed to lock RTRB consumer",
                                log_prefix, device_id
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
                    &samples_buffer
                };

                if samples.is_empty() {
                    continue;
                }

                // Step 1: Check if resampling is needed
                let sample_rate_difference =
                    (device_sample_rate as f32 - initial_target_sample_rate as f32).abs();
                let needs_resampling = sample_rate_difference > 1.0; // Allow 1Hz tolerance

                // Step 2: Pre-accumulate incoming samples
                input_accumulator.extend_from_slice(&samples);

                // Step 3: Process all available chunks from the accumulator
                loop {
                    let accumulated_samples = Self::process_with_pre_accumulation(
                        &mut resampler,
                        needs_resampling,
                        &[],
                        &mut input_accumulator,
                        chunk_size,
                        device_id.clone(),
                    );

                    // If we don't have enough accumulated samples, break inner loop
                    let accumulated_samples = match accumulated_samples {
                        Some(samples) => samples,
                        None => break, // Not enough samples, wait for more input
                    };

                    let processing_start = std::time::Instant::now();

                    // Step 4: Resample accumulated samples if needed, otherwise pass through
                    let resample_start = std::time::Instant::now();
                    let processed_samples = if needs_resampling {
                        if let Some(active_resampler) = Self::get_or_initialize_resampler_static(
                            &mut resampler,
                            device_sample_rate,
                            initial_target_sample_rate,
                            chunk_size,
                            channels,
                            &device_id,
                        ) {
                            // Resample the accumulated samples
                            let resampled = active_resampler.convert(&accumulated_samples);

                            // Apply dynamic rate adjustment
                            let _ = Self::adjust_dynamic_sample_rate(
                                active_resampler,
                                &queue_tracker,
                                device_sample_rate,
                                initial_target_sample_rate,
                                &device_id,
                            );

                            static RESAMPLE_LOG_COUNT: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let resample_count = RESAMPLE_LOG_COUNT
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                            if resample_count < 10 || resample_count % 1000 == 0 {
                                info!(
                                    "🔄 {}: Resampled {} input → {} output ({}Hz→{}Hz) {}",
                                    "AUDIO_RESAMPLE".on_cyan().white(),
                                    accumulated_samples.len(),
                                    resampled.len(),
                                    device_sample_rate,
                                    initial_target_sample_rate,
                                    device_id
                                );
                            }

                            resampled
                        } else {
                            warn!(
                                "⚠️ {}[{}]: Failed to initialize resampler, passing through",
                                log_prefix, device_id
                            );
                            accumulated_samples
                        }
                    } else {
                        // No resampling needed - pass through accumulated samples
                        accumulated_samples
                    };
                    let resample_duration = resample_start.elapsed();

                    // Step 5: Apply post-processing (effects, VU meters, etc.) if provided
                    let post_process_start = std::time::Instant::now();
                    let mut final_samples = processed_samples;
                    if let Some(ref mut process_fn) = post_process_fn {
                        if let Err(e) = process_fn(&mut final_samples, channels, &device_id) {
                            warn!(
                                "⚠️ {}[{}]: Post-processing failed: {}",
                                log_prefix, device_id, e
                            );
                        }
                    }
                    let post_process_duration = post_process_start.elapsed();

                    // Step 6: Write to output RTRB queue
                    let write_start = std::time::Instant::now();
                    Self::write_samples_to_rtrb_sync(
                        &device_id,
                        &final_samples,
                        &rtrb_producer,
                        Some(&queue_tracker),
                    );
                    let write_duration = write_start.elapsed();

                    samples_processed += 1;
                    let processing_duration = processing_start.elapsed();

                    // Rate-limited logging
                    if samples_processed <= 5 || samples_processed % 1000 == 0 {
                        info!(
                            "🔄 {}: {} processed {} samples in {}μs (resample: {}μs, post: {}μs, write: {}μs) batch #{}",
                            log_prefix.on_cyan().white(),
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
                            log_prefix.on_cyan().white(),
                            device_id,
                            processing_duration.as_micros(),
                            resample_duration.as_micros(),
                            post_process_duration.as_micros(),
                            write_duration.as_micros()
                        );
                    }
                }
            }
        });

        self.set_worker_handle(worker_handle);
        info!(
            "✅ {}: Started worker thread for device '{}'",
            self.log_prefix().on_cyan().white(),
            self.device_id()
        );

        Ok(())
    }

    /// Get queue tracker for sharing with other components
    fn get_queue_tracker_for_consumer(&self) -> AtomicQueueTracker {
        self.queue_tracker().clone()
    }

    /// Update target mix rate
    fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        // Capture values before borrowing resampler mutably
        let output_rate = self.device_sample_rate() as f32;
        let new_input_rate = target_mix_rate as f32;
        let device_id = self.device_id().to_string();

        if let Some(ref mut resampler) = self.resampler_mut() {
            // Use dynamic adjustment - keep same output rate, update input rate
            match resampler.set_sample_rates(new_input_rate, output_rate, true, device_id.clone()) {
                Ok(()) => {
                    info!(
                        "🎯 {}: Dynamic rate update for {} - {}Hz→{}Hz (ratio: {:.6})",
                        "DYNAMIC_RATE_UPDATE".on_cyan().white(),
                        device_id,
                        new_input_rate,
                        output_rate,
                        output_rate / new_input_rate
                    );
                    self.set_target_sample_rate(target_mix_rate);
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        "⚠️ {}: Dynamic rate update failed for {}: {}, falling back to recreation",
                        "DYNAMIC_RATE_FAILED".on_cyan().white(),
                        device_id,
                        e
                    );
                }
            }
        }

        // Fallback: force resampler recreation on next processing cycle
        self.set_resampler(None);
        self.set_target_sample_rate(target_mix_rate);
        info!(
            "🔧 {}: Marked resampler for recreation - new mix rate: {}Hz",
            "RESAMPLER_RESET".on_cyan().white(),
            target_mix_rate
        );
        Ok(())
    }

    /// Pre-accumulation strategy: Collect enough input samples before resampling
    /// accumulate input frames until we have enough to produce target output
    fn process_with_pre_accumulation(
        resampler: &mut Option<RubatoSRC>,
        needs_resampling: bool,
        _input_samples: &[f32],
        accumulation_buffer: &mut Vec<f32>,
        target_output_samples: usize,
        device_id: String,
    ) -> Option<Vec<f32>> {
        // Check if we have enough input to produce target output
        let input_frames_needed = if needs_resampling {
            if let Some(ref mut active_resampler) = resampler {
                let output_frames = target_output_samples / 2;
                active_resampler.input_frames_needed(output_frames) * 2
            } else {
                target_output_samples
            }
        } else {
            target_output_samples
        };

        // If we have enough, extract and return the samples
        if accumulation_buffer.len() >= input_frames_needed {
            // Extract the samples we need
            let to_process: Vec<f32> = accumulation_buffer.drain(..input_frames_needed).collect();
            Some(to_process)
        } else {
            // Not enough input yet, keep accumulating
            None
        }
    }

    /// Dynamic sample rate adjustment using queue tracker to prevent drift
    /// Monitors queue fill levels and adjusts resampling ratio accordingly
    fn adjust_dynamic_sample_rate(
        resampler: &mut RubatoSRC,
        queue_tracker: &AtomicQueueTracker,
        input_sample_rate: u32,
        device_sample_rate: u32,
        device_id: &str,
    ) -> Result<(), String> {
        // Get adjusted ratio from queue tracker
        let adjusted_ratio = queue_tracker.adjust_ratio(input_sample_rate, device_sample_rate);
        let new_out_rate = input_sample_rate as f32 * adjusted_ratio;

        // Apply the adjusted sample rate
        resampler
            .set_sample_rates(
                input_sample_rate as f32,
                new_out_rate,
                true,
                device_id.to_string(),
            )
            .map_err(|err| {
                warn!("⚠️ Drift correction failed: {}", err);
                err.to_string()
            })
    }

    /// Stop the worker
    async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.take_worker_handle() {
            handle.abort();

            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => info!(
                    "✅ {}: '{}' shut down gracefully",
                    self.log_prefix(),
                    self.device_id()
                ),
                Err(_) => warn!(
                    "⚠️ {}: '{}' force-terminated after timeout",
                    self.log_prefix(),
                    self.device_id()
                ),
            }
        }

        Ok(())
    }
}
