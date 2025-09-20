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

// Removed ResampledAudioChunk - no longer needed for single-thread architecture

/// Output processing worker for a specific device with single-thread architecture
pub struct OutputWorker {
    device_id: String,
    pub device_sample_rate: u32, // Target device sample rate (e.g., 44.1kHz)

    // Audio processing components (used only for initialization)
    target_chunk_size: usize, // Device-required buffer size (e.g., 512 samples stereo)

    // Communication channels
    mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,

    // Hardware buffer size updates (macOS CoreAudio only)
    #[cfg(target_os = "macos")]
    hardware_update_tx: Option<mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>>,

    // Hardware output integration via SPMC queue
    spmc_writer: Option<Arc<Mutex<Writer<f32>>>>, // Writes to hardware via SPMC queue

    // Single worker thread handle
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
            target_chunk_size,
            mixed_audio_rx,
            hardware_update_tx: Some(hardware_update_tx),
            spmc_writer,
            worker_handle: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    pub fn update_target_mix_rate(&mut self, _target_mix_rate: u32) -> Result<()> {
        // **TWO-THREAD ARCHITECTURE**: Resampler recreation is handled per-thread
        // Each producer thread manages its own resampler state
        info!("🔄 OUTPUT_WORKER: '{}' target mix rate updated (threads will adapt automatically)", self.device_id);
        Ok(())
    }

    /// Static helper function to get or initialize resampler in async context
    fn get_or_initialize_resampler_static<'a>(
        resampler: &'a mut Option<RubatoSRC>,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize, // Output device chunk size
        device_id: &str,
    ) -> Option<&'a mut RubatoSRC> {
        let sample_rate_difference = (input_sample_rate as f32 - output_sample_rate as f32).abs();

        // No resampling needed if rates are close (within 1 Hz)
        if sample_rate_difference <= 1.0 {
            return None;
        }

        // Check if resampler exists and has the correct rates
        let needs_recreation = if let Some(ref existing_resampler) = resampler {
            existing_resampler.input_rate() != input_sample_rate as f32
                || existing_resampler.output_rate() != output_sample_rate as f32
        } else {
            true // No resampler exists
        };

        // Create or recreate resampler if needed
        if needs_recreation {
            // **NEW FIFO APPROACH**: chunk_size is target output chunk size (samples)
            let output_frames_per_channel = chunk_size / 2; // Stereo: 2 samples per frame
            match RubatoSRC::new_sinc_fixed_out(
                input_sample_rate as f32,
                output_sample_rate as f32,
                output_frames_per_channel, // Fixed output size in frames per channel
                2, // channels (stereo)
            ) {
                Ok(new_resampler) => {
                    info!(
                        "🔄 {}: {} SINC_FIXED_OUT resampler for {} ({} Hz → {} Hz, {} output_frames_per_channel)",
                        "OUTPUT_RESAMPLER".green(),
                        if resampler.is_some() {
                            "Recreated"
                        } else {
                            "Created"
                        },
                        device_id,
                        input_sample_rate,
                        output_sample_rate,
                        output_frames_per_channel
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



    /// Start the single-thread output processing worker
    pub fn start(&mut self) -> Result<()> {
        let device_id = self.device_id.clone();
        let device_sample_rate = self.device_sample_rate;
        let target_chunk_size = self.target_chunk_size;

        // Take ownership of mixed audio receiver
        let mut mixed_audio_rx =
            std::mem::replace(&mut self.mixed_audio_rx, mpsc::unbounded_channel().1);
        let spmc_writer = self.spmc_writer.clone();

        info!(
            "🚀 OUTPUT_WORKER: Starting single-thread architecture for device '{}'",
            device_id
        );

        // **SINGLE THREAD**: Process mixed audio -> resample -> write to SPMC
        let worker_handle = tokio::spawn(async move {
            let mut resampler: Option<RubatoSRC> = None;
            let mut chunks_processed = 0u64;

            info!(
                "🚀 SINGLE_THREAD_WORKER: Started for device '{}'",
                device_id
            );

            while let Some(mixed_audio) = mixed_audio_rx.recv().await {
                let processing_start = std::time::Instant::now();
                chunks_processed += 1;

                // **STEP 1: RESAMPLE** (if needed)
                let resampled_samples = if let Some(active_resampler) =
                    Self::get_or_initialize_resampler_static(
                        &mut resampler,
                        mixed_audio.sample_rate,
                        device_sample_rate,
                        target_chunk_size,
                        &device_id,
                    ) {
                    // Push samples into resampler FIFO and pull output
                    active_resampler.push_interleaved(&mixed_audio.samples);
                    let output_frames = target_chunk_size / 2; // stereo frames
                    active_resampler.get_output_interleaved(output_frames)
                } else {
                    // No resampling needed
                    mixed_audio.samples.clone()
                };

                // **STEP 2: WRITE TO SPMC** (directly, no accumulation buffer)
                let spmc_write_start = std::time::Instant::now();
                if let Some(ref spmc_writer) = spmc_writer {
                    Self::write_to_hardware_spmc(
                        &device_id,
                        &resampled_samples,
                        spmc_writer,
                    )
                    .await;
                }
                let spmc_write_duration = spmc_write_start.elapsed();
                let total_duration = processing_start.elapsed();

                // Rate-limited logging
                if chunks_processed <= 5 || chunks_processed % 1000 == 0 {
                    info!(
                        "🎵 {}: {} processed chunk #{} ({} samples) in {}μs (spmc: {}μs)",
                        "SINGLE_THREAD_WORKER".yellow(),
                        device_id,
                        chunks_processed,
                        resampled_samples.len(),
                        total_duration.as_micros(),
                        spmc_write_duration.as_micros()
                    );
                }

                // Performance monitoring
                if total_duration.as_micros() > 3000 {
                    warn!(
                        "⏱️ SINGLE_THREAD_SLOW: {} slow processing: {}μs",
                        device_id,
                        total_duration.as_micros()
                    );
                }
            }

            info!(
                "🛑 SINGLE_THREAD_WORKER: Thread for '{}' shutting down (processed {} chunks)",
                device_id, chunks_processed
            );
        });

        self.worker_handle = Some(worker_handle);
        info!(
            "✅ OUTPUT_WORKER: Started single-thread worker for device '{}'",
            self.device_id
        );

        Ok(())
    }

    /// Stop the output processing worker (single thread)
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();

            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => info!("✅ OUTPUT_WORKER: '{}' shut down gracefully", self.device_id),
                Err(_) => warn!(
                    "⚠️ OUTPUT_WORKER: '{}' force-terminated after timeout",
                    self.device_id
                ),
            }
        }

        Ok(())
    }

    /// OLD STOP METHOD - COMMENTED OUT
    /*
    pub async fn stop_old(&mut self) -> Result<()> {
        let mut shutdown_results = Vec::new();

        // Stop resampling producer thread
        if let Some(handle) = self.resampling_thread_handle.take() {
            handle.abort();
            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => {
                    info!("✅ OUTPUT_WORKER: '{}' resampling producer shut down gracefully", self.device_id);
                    shutdown_results.push("producer: graceful");
                },
                Err(_) => {
                    warn!("⚠️ OUTPUT_WORKER: '{}' resampling producer force-terminated after timeout", self.device_id);
                    shutdown_results.push("producer: timeout");
                }
            }
        }

        // Stop hardware consumer thread
        if let Some(handle) = self.hardware_thread_handle.take() {
            handle.abort();
            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => {
                    info!("✅ OUTPUT_WORKER: '{}' hardware consumer shut down gracefully", self.device_id);
                    shutdown_results.push("consumer: graceful");
                },
                Err(_) => {
                    warn!("⚠️ OUTPUT_WORKER: '{}' hardware consumer force-terminated after timeout", self.device_id);
                    shutdown_results.push("consumer: timeout");
                }
            }
        }

        info!("🛑 OUTPUT_WORKER: '{}' two-thread architecture stopped ({})",
              self.device_id, shutdown_results.join(", "));

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
    */

    /// Get processing statistics
    pub fn get_stats(&self) -> OutputWorkerStats {
        OutputWorkerStats {
            device_id: self.device_id.clone(),
            chunks_processed: self.chunks_processed,
            samples_output: self.samples_output,
            buffer_size: 0, // Buffer size managed per-thread
            target_chunk_size: self.target_chunk_size,
            is_running: self.worker_handle.is_some(),
        }
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

            for &sample in samples.iter() {
                writer.write(sample);
                samples_written += 1;
            }

            if samples_written != samples.len() {
                warn!(
                    "⚠️ OUTPUT_WORKER: {} SPMC queue full, wrote only {}/{} samples",
                    device_id,
                    samples_written,
                    samples.len()
                );
            }

            // Optional performance logging
            if lock_duration.as_micros() > 100 {
                warn!(
                    "⏱️ OUTPUT_WORKER: {} SPMC lock took {}μs",
                    device_id,
                    lock_duration.as_micros()
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
