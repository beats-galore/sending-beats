// Layer 4: Output Processing Workers
//
// Each output device gets its own dedicated worker thread that:
// 1. Receives mixed audio from Layer 3 mixing
// 2. Resamples from max rate to device-specific rate
// 3. Buffers samples to proper chunk sizes for hardware
// 4. Sends audio to actual output devices

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::queue_types::MixedAudioSamples;
use crate::audio::mixer::sample_rate_converter::RubatoSRC;

/// Output processing worker for a specific device
pub struct OutputWorker {
    device_id: String,
    device_sample_rate: u32, // Target device sample rate (e.g., 44.1kHz)

    // Audio processing components
    resampler: Option<RubatoSRC>,
    sample_buffer: Vec<f32>,  // Hardware chunk accumulator
    target_chunk_size: usize, // Device-required buffer size (e.g., 512 samples stereo)

    // Communication channels
    mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,

    // Device output integration
    // TODO: Add actual device output sender when we integrate with CoreAudio/CPAL

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
        info!(
            "🔊 OUTPUT_WORKER: Creating worker for device '{}' ({} Hz, {} sample chunks)",
            device_id, device_sample_rate, target_chunk_size
        );

        Self {
            device_id,
            device_sample_rate,
            resampler: None,
            sample_buffer: Vec::new(),
            target_chunk_size,
            mixed_audio_rx,
            worker_handle: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    /// Start the output processing worker thread
    pub fn start(&mut self) -> Result<()> {
        let device_id = self.device_id.clone();
        let device_sample_rate = self.device_sample_rate;
        let target_chunk_size = self.target_chunk_size;

        // Take ownership of receiver for the worker thread
        let mut mixed_audio_rx =
            std::mem::replace(&mut self.mixed_audio_rx, mpsc::unbounded_channel().1);

        // Spawn dedicated worker thread
        let worker_handle = tokio::spawn(async move {
            let mut resampler: Option<RubatoSRC> = None;
            let mut sample_buffer = Vec::new();
            let mut chunks_processed = 0u64;

            info!(
                "🚀 OUTPUT_WORKER: Started processing thread for device '{}'",
                device_id
            );

            while let Some(mixed_audio) = mixed_audio_rx.recv().await {
                let processing_start = std::time::Instant::now();

                // Step 1: Resample from mix rate to device rate if needed
                let device_samples = if (mixed_audio.sample_rate as f32 - device_sample_rate as f32)
                    .abs()
                    > 1.0
                {
                    // Create resampler if needed (persistent across calls)
                    if resampler.is_none() {
                        match RubatoSRC::new_low_artifact(
                            mixed_audio.sample_rate as f32,
                            device_sample_rate as f32,
                        ) {
                            Ok(new_resampler) => {
                                info!(
                                    "🔧 OUTPUT_WORKER: Created resampler for {} ({} Hz → {} Hz)",
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

                    // Resample using persistent resampler with accumulator logic
                    if let Some(ref mut resampler) = resampler {
                        let target_samples = resampler.get_target_chunk_size() * 2; // Stereo
                        let accumulator_size = resampler.get_accumulator_size();

                        if accumulator_size >= target_samples {
                            // Accumulator has enough samples - drain for hardware output
                            resampler.drain_accumulator_only()
                        } else {
                            // Accumulator needs more samples - process normally
                            resampler.convert(&mixed_audio.samples)
                        }
                    } else {
                        mixed_audio.samples
                    }
                } else {
                    // No resampling needed
                    mixed_audio.samples
                };

                // Step 2: Accumulate samples until we have a proper hardware chunk
                sample_buffer.extend(device_samples);

                // Step 3: Send hardware-sized chunks to device
                while sample_buffer.len() >= target_chunk_size {
                    // Extract chunk for hardware
                    let hardware_chunk: Vec<f32> =
                        sample_buffer.drain(..target_chunk_size).collect();

                    // TODO: Send to actual audio output device
                    // For now, just simulate the device output
                    Self::simulate_device_output(&device_id, &hardware_chunk);

                    chunks_processed += 1;

                    // Rate-limited logging
                    if chunks_processed <= 5 || chunks_processed % 100 == 0 {
                        info!(
                            "🎵 OUTPUT_WORKER: {} sent chunk #{} ({} samples) to device",
                            device_id,
                            chunks_processed,
                            hardware_chunk.len()
                        );
                    }
                }

                let processing_duration = processing_start.elapsed();

                // Performance monitoring
                if processing_duration.as_micros() > 500 {
                    warn!(
                        "⏱️ OUTPUT_WORKER: {} slow processing: {}μs",
                        device_id,
                        processing_duration.as_micros()
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

    /// Simulate device output (placeholder until we integrate with actual audio output)
    fn simulate_device_output(device_id: &str, samples: &[f32]) {
        // Calculate peak level for monitoring
        let peak_level = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        // Very rate-limited logging to avoid spam
        use std::sync::atomic::{AtomicU64, Ordering};
        static OUTPUT_COUNT: AtomicU64 = AtomicU64::new(0);
        let count = OUTPUT_COUNT.fetch_add(1, Ordering::Relaxed);

        if count <= 3 || count % 1000 == 0 {
            info!(
                "🎧 DEVICE_OUTPUT: {} received {} samples, peak: {:.3} (output #{})",
                device_id,
                samples.len(),
                peak_level,
                count
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
