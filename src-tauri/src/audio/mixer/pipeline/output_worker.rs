// Layer 4: Output Processing Workers
//
// Each output device gets its own dedicated worker thread that:
// 1. Receives mixed audio from Layer 3 mixing
// 2. Resamples from max rate to device-specific rate
// 3. Buffers samples to proper chunk sizes for hardware
// 4. Sends audio to actual output devices

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use super::queue_types::MixedAudioSamples;
use crate::audio::mixer::resampling::R8brainSRC;
use crate::audio::utils::calculate_optimal_chunk_size;
use colored::*;

// SPMC queue imports for hardware output
use spmcq::Writer;

/// Output processing worker for a specific device
pub struct OutputWorker {
    device_id: String,
    pub device_sample_rate: u32, // Target device sample rate (e.g., 44.1kHz)

    // Audio processing components
    resampler: Option<R8brainSRC>,
    sample_buffer: Vec<f32>,  // Hardware chunk accumulator
    target_chunk_size: usize, // Device-required buffer size (e.g., 512 samples stereo)

    // **ACCUMULATION**: Buffer for collecting variable FftFixedIn outputs until hardware chunk size
    accumulation_buffer: Vec<f32>, // Accumulates samples until target_chunk_size reached

    // Communication channels
    mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,

    // Hardware buffer size updates (macOS CoreAudio only)
    #[cfg(target_os = "macos")]
    hardware_update_tx: Option<mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>>,

    // Hardware output integration via SPMC queue
    spmc_writer: Option<Arc<Mutex<Writer<f32>>>>, // Writes to hardware via SPMC queue

    // Worker thread handle
    worker_handle: Option<tokio::task::JoinHandle<()>>,

    // Performance metrics
    chunks_processed: u64,
    samples_output: u64,
}

impl OutputWorker {
    /// Create a new output processing worker for a device
    pub fn new(
        device_id: String,
        device_sample_rate: u32,
        target_chunk_size: usize,
        mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,
    ) -> Self {
        Self::new_with_spmc_writer(
            device_id,
            device_sample_rate,
            target_chunk_size,
            mixed_audio_rx,
            None,
        )
    }

    /// Create a new output processing worker with SPMC writer for hardware output
    pub fn new_with_spmc_writer(
        device_id: String,
        device_sample_rate: u32,
        target_chunk_size: usize,
        mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,
        spmc_writer: Option<Arc<Mutex<Writer<f32>>>>,
    ) -> Self {
        let has_hardware_output = spmc_writer.is_some();
        info!(
            "🔊 OUTPUT_WORKER: Creating worker for device '{}' ({} Hz, {} sample chunks, hardware: {})",
            device_id, device_sample_rate, target_chunk_size, has_hardware_output
        );

        Self {
            device_id,
            device_sample_rate,
            resampler: None,
            sample_buffer: Vec::new(),
            target_chunk_size,
            accumulation_buffer: Vec::with_capacity(target_chunk_size * 2), // Pre-allocate for efficiency
            mixed_audio_rx,
            #[cfg(target_os = "macos")]
            hardware_update_tx: None, // No hardware updates for this constructor
            spmc_writer,
            worker_handle: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    /// Create a new output processing worker with hardware update channel (macOS only)
    #[cfg(target_os = "macos")]
    pub fn new_with_hardware_updates(
        device_id: String,
        device_sample_rate: u32,
        target_chunk_size: usize,
        mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,
        spmc_writer: Option<Arc<Mutex<Writer<f32>>>>,
        hardware_update_tx: mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
    ) -> Self {
        let has_hardware_output = spmc_writer.is_some();
        info!(
            "🔊 OUTPUT_WORKER: Creating worker with hardware updates for device '{}' ({} Hz, {} sample chunks, hardware: {})",
            device_id, device_sample_rate, target_chunk_size, has_hardware_output
        );

        Self {
            device_id,
            device_sample_rate,
            resampler: None,
            sample_buffer: Vec::new(),
            target_chunk_size,
            accumulation_buffer: Vec::with_capacity(target_chunk_size * 2), // Pre-allocate for efficiency
            mixed_audio_rx,
            hardware_update_tx: Some(hardware_update_tx),
            spmc_writer,
            worker_handle: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    pub fn update_target_mix_rate(&mut self, _target_mix_rate: u32) -> Result<()> {
        // **CRITICAL**: Force resampler recreation with new target rate
        // The resampler will be recreated in the worker thread when needed
        self.resampler = None;
        Ok(())
    }

    /// Static helper function to get or initialize resampler in async context
    fn get_or_initialize_resampler_static<'a>(
        resampler: &'a mut Option<R8brainSRC>,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize, // Output device chunk size
        device_id: &str,
    ) -> Option<&'a mut R8brainSRC> {
        let sample_rate_difference = (input_sample_rate as f32 - output_sample_rate as f32).abs();

        // No resampling needed if rates are close (within 1 Hz)
        if sample_rate_difference <= 1.0 {
            return None;
        }

        // Check if resampler exists and has the correct rates
        let needs_recreation = if let Some(ref existing_resampler) = resampler {
            existing_resampler.input_rate() != input_sample_rate
                || existing_resampler.output_rate() != output_sample_rate
        } else {
            true // No resampler exists
        };

        // Create or recreate resampler if needed
        if needs_recreation {
            // Calculate buffer size based on device callback rate
            // Buffer should be 2-3x the device chunk size to prevent underruns
            let device_chunk_duration_ms = (chunk_size as f32 / output_sample_rate as f32) * 1000.0;
            let buffer_size_ms = device_chunk_duration_ms * 3.0; // 3x chunk size for safety

            info!(
                "🔄 {}: Device callback: {} samples = {:.1}ms, buffer: {:.1}ms",
                "OUTPUT_BUFFER_CALC".cyan(),
                chunk_size,
                device_chunk_duration_ms,
                buffer_size_ms
            );

            // Using R8brainSRC with continuous read pointer philosophy
            match R8brainSRC::new(
                input_sample_rate,
                output_sample_rate,
                buffer_size_ms,
            ) {
                Ok(new_resampler) => {
                    info!(
                        "🔄 {}: {} resampler for {} ({} Hz → {} Hz, ratio: {:.3})",
                        "OUTPUT_RESAMPLER".green(),
                        if resampler.is_some() {
                            "Recreated"
                        } else {
                            "Created"
                        },
                        device_id,
                        input_sample_rate,
                        output_sample_rate,
                        new_resampler.ratio()
                    );
                    *resampler = Some(new_resampler);
                }
                Err(e) => {
                    error!(
                        "❌ OUTPUT_WORKER: Failed to create resampler for {}: {}",
                        device_id, e
                    );
                    return None;
                }
            }
        }

        // Return mutable reference to the resampler
        resampler.as_mut()
    }

    /// Start the output processing worker thread
    pub fn start(&mut self) -> Result<()> {
        let device_id = self.device_id.clone();
        let device_sample_rate = self.device_sample_rate;
        let target_chunk_size = self.target_chunk_size;

        // Take ownership of receiver and SPMC writer for the worker thread
        let mut mixed_audio_rx =
            std::mem::replace(&mut self.mixed_audio_rx, mpsc::unbounded_channel().1);
        let spmc_writer = self.spmc_writer.clone();

        // Clone hardware update channel for dynamic buffer size updates
        #[cfg(target_os = "macos")]
        let hardware_update_tx = self.hardware_update_tx.clone();

        // Spawn dedicated worker thread
        let worker_handle = tokio::spawn(async move {
            let mut resampler: Option<R8brainSRC> = None;
            let mut chunks_processed = 0u64;
            let mut adaptive_chunk_size = target_chunk_size; // Start with default, adapt on first audio

            // **ACCUMULATION BUFFER**: Collect variable FftFixedIn outputs until hardware chunk size
            let mut accumulation_buffer: Vec<f32> = Vec::with_capacity(adaptive_chunk_size * 2);

            info!(
                "🚀 OUTPUT_WORKER: Started processing thread for device '{}'",
                device_id
            );

            while let Some(mixed_audio) = mixed_audio_rx.recv().await {
                let processing_start = std::time::Instant::now();
                let receive_time = processing_start;

                // **DYNAMIC CHUNK SIZING**: Recalculate chunk size whenever input sample rate changes
                static mut LAST_INPUT_SAMPLE_RATE: Option<u32> = None;
                let input_rate_changed = unsafe {
                    let changed = LAST_INPUT_SAMPLE_RATE
                        .map_or(true, |last_rate| last_rate != mixed_audio.sample_rate);
                    LAST_INPUT_SAMPLE_RATE = Some(mixed_audio.sample_rate);
                    changed
                };

                if input_rate_changed {
                    let optimal_chunk_size = calculate_optimal_chunk_size(
                        mixed_audio.sample_rate,
                        device_sample_rate,
                        target_chunk_size,
                    );
                    if optimal_chunk_size != adaptive_chunk_size {
                        adaptive_chunk_size = optimal_chunk_size;
                        info!("🔧 DYNAMIC_CHUNKS: {} updated chunk size to {} for {}Hz→{}Hz (sample rate changed)",
                              device_id, adaptive_chunk_size, mixed_audio.sample_rate, device_sample_rate);

                        // **HARDWARE SYNC**: Update CoreAudio hardware buffer size to match
                        #[cfg(target_os = "macos")]
                        if let Some(ref hardware_tx) = hardware_update_tx {
                            let command = crate::audio::mixer::stream_management::AudioCommand::UpdateOutputHardwareBufferSize {
                                device_id: device_id.clone(),
                                target_frames: adaptive_chunk_size as u32,
                            };
                            if let Err(e) = hardware_tx.try_send(command) {
                                warn!("⚠️ Failed to send hardware buffer update: {}", e);
                            } else {
                                info!(
                                    "📡 {}: Sent hardware buffer update to {} frames",
                                    "HARDWARE_SYNC_COMMAND".cyan(),
                                    adaptive_chunk_size
                                );
                            }
                        }
                    }
                }

                // Capture input size before samples are moved
                let input_samples_len = mixed_audio.samples.len();

                // **TIMING DEBUG**: Track time between receives
                static mut LAST_RECEIVE_TIME: Option<std::time::Instant> = None;
                let time_since_last = unsafe {
                    if let Some(last_time) = LAST_RECEIVE_TIME {
                        Some(receive_time.duration_since(last_time))
                    } else {
                        None
                    }
                };
                unsafe {
                    LAST_RECEIVE_TIME = Some(receive_time);
                }

                // Step 1: Resample from mix rate to device rate if needed
                let resample_start = std::time::Instant::now();
                let rate_ratio = mixed_audio.sample_rate as f32 / device_sample_rate as f32;

                let device_samples = if let Some(active_resampler) =
                    Self::get_or_initialize_resampler_static(
                        &mut resampler,
                        mixed_audio.sample_rate,
                        device_sample_rate,
                        adaptive_chunk_size,
                        &device_id,
                    ) {
                    let resampler_start = std::time::Instant::now();

                    // Step 1: Add input samples to the continuous buffer (the "tape")
                    active_resampler.add_input_samples(&mixed_audio.samples);

                    // Step 2: Read exactly the number of samples we need (the "player head")
                    let resampled = active_resampler.read_output_samples(adaptive_chunk_size);

                    let resampler_duration = resampler_start.elapsed();

                    // Log resampling operation timing
                    static RESAMPLE_LOG_COUNT: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let resample_count =
                        RESAMPLE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    // Performance warning logs (for slow operations)
                    if (resample_count < 20 || resample_count % 500 == 0)
                        && resampler_duration.as_micros() > 500
                    {
                        info!(
                            "🔄 {}: {} continuous resample: added {} → read {} samples in {}μs ({}Hz→{}Hz, ratio: {:.3}, buffer: {:.1}%)",
                            "R8BRAIN_TIMING".cyan(),
                            device_id,
                            mixed_audio.samples.len(),
                            resampled.len(),
                            resampler_duration.as_micros(),
                            mixed_audio.sample_rate,
                            device_sample_rate,
                            rate_ratio,
                            active_resampler.buffer_fill_ratio() * 100.0
                        );
                    }

                    // Periodic status logs (every 1000 cycles regardless of performance)
                    if resample_count < 5 || resample_count % 1000 == 0 {
                        info!(
                            "🔄 {}: {} periodic status: {} → {} samples in {}μs, buffer: {:.1}%, ratio: {:.3}",
                            "R8BRAIN_STATUS".cyan(),
                            device_id,
                            mixed_audio.samples.len(),
                            resampled.len(),
                            resampler_duration.as_micros(),
                            active_resampler.buffer_fill_ratio() * 100.0,
                            rate_ratio
                        );
                    }

                    resampled
                } else {
                    // No resampling needed - use original samples
                    mixed_audio.samples
                };
                let resample_duration = resample_start.elapsed();

                // **OPTIMIZATION**: If no resampling and chunk size matches, bypass accumulation entirely
                let mut chunks_sent_this_cycle = 0;
                let mut total_spmc_duration = std::time::Duration::ZERO;

                if !device_samples.is_empty() {
                    // **DIRECT STREAMING**: Send R8brain output directly to SPMC queue
                    // Let CoreAudio callback pull exactly what it needs when it needs it
                    let spmc_write_start = std::time::Instant::now();
                    if let Some(ref spmc_writer) = spmc_writer {
                        Self::write_to_hardware_spmc(&device_id, &device_samples, spmc_writer).await;
                    }
                    let spmc_write_duration = spmc_write_start.elapsed();
                    total_spmc_duration += spmc_write_duration;

                    chunks_processed += 1;
                    chunks_sent_this_cycle += 1;

                    // Rate-limited logging for streaming
                    if chunks_processed <= 5 || chunks_processed % 1000 == 0 {
                        info!(
                            "🎵 {} (4th layer): {} streamed batch #{} ({} samples) 🌊DIRECT_STREAM",
                            "OUTPUT_WORKER".purple(),
                            device_id,
                            chunks_processed,
                            device_samples.len()
                        );
                    }

                }

                let processing_duration = processing_start.elapsed();

                // **COMPREHENSIVE TIMING DIAGNOSTICS** for downsampling stuttering
                static TIMING_DEBUG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let debug_count =
                    TIMING_DEBUG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                if (debug_count < 10 || debug_count % 1000 == 0)
                    && processing_duration.as_micros() > 1000
                {
                    let time_between = if let Some(gap) = time_since_last {
                        format!("{}μs", gap.as_micros())
                    } else {
                        "N/A".to_string()
                    };

                    info!("⏱️  {} [{}]: gap_since_last={}, input={}→{} samples, 🔄resample={}μs, chunks_sent={}, spmc={}μs, total={}μs (FFT_FIXED_IN)",
                        "OUTPUT_TIMING".purple(),
                        device_id,
                        time_between,
                        input_samples_len,
                        device_samples.len(),
                        resample_duration.as_micros(),
                        chunks_sent_this_cycle,
                        total_spmc_duration.as_micros(),
                        processing_duration.as_micros()
                    );
                }

                // Performance monitoring
                use std::sync::atomic::{AtomicU64, Ordering};
                static OUTPUT_WORKER_COUNT: AtomicU64 = AtomicU64::new(0);
                let count = OUTPUT_WORKER_COUNT.fetch_add(1, Ordering::Relaxed);
                if processing_duration.as_micros() > 500 && (count <= 3 || count % 1000 == 0) {
                    warn!(
                        "🐌 {}: {} SLOW processing: {}μs (🔄resample: {}μs, spmc: {}μs) [FFT_FIXED_IN]",
                        "OUTPUT_WORKER_SLOW".bright_red(),
                        device_id,
                        processing_duration.as_micros(),
                        resample_duration.as_micros(),
                        total_spmc_duration.as_micros()
                    );
                }
            }

            info!(
                "🛑 OUTPUT_WORKER: Processing thread for '{}' shutting down (processed {} chunks)",
                device_id, chunks_processed
            );
        });

        self.worker_handle = Some(worker_handle);
        info!(
            "✅ OUTPUT_WORKER: Started worker thread for device '{}'",
            self.device_id
        );

        Ok(())
    }

    /// Stop the output processing worker
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();

            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => info!(
                    "✅ OUTPUT_WORKER: '{}' shut down gracefully",
                    self.device_id
                ),
                Err(_) => warn!(
                    "⚠️ OUTPUT_WORKER: '{}' force-terminated after timeout",
                    self.device_id
                ),
            }
        }

        Ok(())
    }

    /// Write audio samples to hardware via SPMC queue
    async fn write_to_hardware_spmc(
        device_id: &str,
        samples: &[f32],
        spmc_writer: &Arc<Mutex<Writer<f32>>>,
    ) {
        let lock_start = std::time::Instant::now();
        if let Ok(mut writer) = spmc_writer.try_lock() {
            let lock_duration = lock_start.elapsed();
            let mut samples_written = 0;
            for &sample in samples {
                writer.write(sample);
                samples_written += 1;
            }

            // Very rate-limited logging to avoid spam
            use std::sync::atomic::{AtomicU64, Ordering};
            static HARDWARE_OUTPUT_COUNT: AtomicU64 = AtomicU64::new(0);
            let count = HARDWARE_OUTPUT_COUNT.fetch_add(1, Ordering::Relaxed);

            if (count <= 3 || count % 1000 == 0) && lock_duration.as_micros() > 100 {
                info!(
                    "🎧 {}: {} wrote {} samples to SPMC queue (lock: {}μs, output #{})",
                    "HARDWARE_OUTPUT".purple(),
                    device_id,
                    samples_written,
                    lock_duration.as_micros(),
                    count
                );
            }
        } else {
            warn!(
                "⚠️ OUTPUT_WORKER: {} failed to lock SPMC writer, dropping {} samples",
                device_id,
                samples.len()
            );
        }
    }

    /// Get processing statistics
    pub fn get_stats(&self) -> OutputWorkerStats {
        OutputWorkerStats {
            device_id: self.device_id.clone(),
            chunks_processed: self.chunks_processed,
            samples_output: self.samples_output,
            buffer_size: self.sample_buffer.len(),
            target_chunk_size: self.target_chunk_size,
            is_running: self.worker_handle.is_some(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutputWorkerStats {
    pub device_id: String,
    pub chunks_processed: u64,
    pub samples_output: u64,
    pub buffer_size: usize,
    pub target_chunk_size: usize,
    pub is_running: bool,
}
