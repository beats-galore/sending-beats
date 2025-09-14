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
use crate::audio::mixer::sample_rate_converter::RubatoSRC;
use crate::audio::utils::calculate_optimal_chunk_size;
use colored::*;

// SPMC queue imports for hardware output
use spmcq::Writer;

/// Output processing worker for a specific device
pub struct OutputWorker {
    device_id: String,
    pub device_sample_rate: u32, // Target device sample rate (e.g., 44.1kHz)

    // Audio processing components
    resampler: Option<RubatoSRC>,
    sample_buffer: Vec<f32>,  // Hardware chunk accumulator
    target_chunk_size: usize, // Device-required buffer size (e.g., 512 samples stereo)

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
            mixed_audio_rx,
            hardware_update_tx: Some(hardware_update_tx),
            spmc_writer,
            worker_handle: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    pub fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        if let Some(ref mut resampler) = self.resampler {
            resampler.update_resampler_rate(target_mix_rate);
        }
        Ok(())
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
            let mut resampler: Option<RubatoSRC> = None;
            let mut sample_buffer = Vec::new();
            let mut chunks_processed = 0u64;
            let mut adaptive_chunk_size = target_chunk_size; // Start with default, adapt on first audio

            // **PERFORMANCE FIX**: Reusable buffer for hardware chunks to avoid allocations
            let mut reusable_hardware_chunk = Vec::with_capacity(target_chunk_size * 2); // Extra capacity for safety

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
                                info!("📡 {}: Sent hardware buffer update to {} frames",
                                    "HARDWARE_SYNC_COMMAND".cyan(), adaptive_chunk_size);
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
                let sample_rate_difference =
                    (mixed_audio.sample_rate as f32 - device_sample_rate as f32).abs();
                let rate_ratio = mixed_audio.sample_rate as f32 / device_sample_rate as f32;
                let device_samples = if (mixed_audio.sample_rate as f32 - device_sample_rate as f32)
                    .abs()
                    > 1.0
                {
                    // Create resampler if needed (persistent across calls)
                    if resampler.is_none() {
                        match RubatoSRC::new_fast(
                            mixed_audio.sample_rate as f32,
                            device_sample_rate as f32,
                        ) {
                            Ok(new_resampler) => {
                                info!(
                                    "🚀 OUTPUT_WORKER: Created FAST resampler for {} ({} Hz → {} Hz)",
                                    device_id, mixed_audio.sample_rate, device_sample_rate
                                );
                                resampler = Some(new_resampler);
                            }
                            Err(e) => {
                                error!(
                                    "❌ OUTPUT_WORKER: Failed to create resampler for {}: {}",
                                    device_id, e
                                );
                                // No resampler created - will use original samples below
                            }
                        };
                    }

                    // Resample using persistent resampler with consistent processing
                    if let Some(ref mut resampler) = resampler {
                        let resampler_convert_start = std::time::Instant::now();
                        // Always process new input to maintain consistent timing
                        let resampled = resampler.convert(&mixed_audio.samples);
                        let resampler_convert_duration = resampler_convert_start.elapsed();

                        // Log actual resampling operation timing
                        static RESAMPLE_LOG_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                        let resample_count = RESAMPLE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        if (resample_count < 5|| resample_count % 2000 == 0)  && resampler_convert_duration.as_micros() > 1000 {
                            info!(
                                "🔄 {}: {} resampled {} → {} samples in {}μs ({}Hz→{}Hz, ratio: {:.3})",
                                "RESAMPLER_TIMING".cyan(),
                                device_id,
                                mixed_audio.samples.len(),
                                resampled.len(),
                                resampler_convert_duration.as_micros(),
                                mixed_audio.sample_rate,
                                device_sample_rate,
                                rate_ratio
                            );
                        }

                        resampled
                    } else {
                        mixed_audio.samples
                    }
                } else {
                    // No resampling needed
                    mixed_audio.samples
                };
                let resample_duration = resample_start.elapsed();

                // Step 2: Accumulate samples until we have a proper hardware chunk
                let buffer_size_before = sample_buffer.len();
                sample_buffer.extend(device_samples);
                let buffer_size_after = sample_buffer.len();
                let buffer_start = std::time::Instant::now();

                // Step 3: Send hardware-sized chunks to device (using adaptive chunk size)
                let mut chunks_sent_this_cycle = 0;
                let mut total_spmc_duration = std::time::Duration::ZERO;
                while sample_buffer.len() >= adaptive_chunk_size {
                    // **PERFORMANCE FIX**: Use reusable buffer instead of allocating new Vec
                    reusable_hardware_chunk.clear();
                    reusable_hardware_chunk.extend(sample_buffer.drain(..adaptive_chunk_size));

                    // Send to actual hardware via SPMC queue
                    let spmc_write_start = std::time::Instant::now();
                    if let Some(ref spmc_writer) = spmc_writer {
                        Self::write_to_hardware_spmc(&device_id, &reusable_hardware_chunk, spmc_writer)
                            .await;
                    } else {
                        // warn!("⚠️ OUTPUT_WORKER: {} has no SPMC writer, dropping {} samples", device_id, reusable_hardware_chunk.len());
                    }
                    let spmc_write_duration = spmc_write_start.elapsed();
                    total_spmc_duration += spmc_write_duration;

                    chunks_processed += 1;
                    chunks_sent_this_cycle += 1;

                    // Rate-limited logging
                    if chunks_processed <= 5 || chunks_processed % 1000 == 0 {
                        info!(
                            "🎵 {} (4th layer): {} sent chunk #{} ({} samples) to device",
                            "OUTPUT_WORKER".purple(),
                            device_id,
                            chunks_processed,
                            reusable_hardware_chunk.len()
                        );
                    }
                }

                let processing_duration = processing_start.elapsed();
                let buffer_duration = buffer_start.elapsed();

                // **COMPREHENSIVE TIMING DIAGNOSTICS** for downsampling stuttering
                static TIMING_DEBUG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let debug_count =
                    TIMING_DEBUG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                if (debug_count < 10 || debug_count % 1000 == 0) && processing_duration.as_micros() > 1000 {
                    let time_between = if let Some(gap) = time_since_last {
                        format!("{}μs", gap.as_micros())
                    } else {
                        "N/A".to_string()
                    };

                    info!("⏱️  {} [{}]: gap_since_last={}, input={}→{} samples, 🔄resample={}μs, buffer={}→{}→{}, chunks_sent={}, spmc={}μs, total={}μs",
                        "OUTPUT_TIMING".purple(),
                        device_id,
                        time_between,
                        input_samples_len,
                        buffer_size_after - buffer_size_before,
                        resample_duration.as_micros(),
                        buffer_size_before,
                        buffer_size_after,
                        sample_buffer.len(),
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
                        "🐌 {}: {} SLOW processing: {}μs (🔄resample: {}μs, buffer: {}μs)",
                        "OUTPUT_WORKER_SLOW".bright_red(),
                        device_id,
                        processing_duration.as_micros(),
                        resample_duration.as_micros(),
                        buffer_duration.as_micros()
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
