// Layer 2: Input Processing Workers
//
// Each input device gets its own dedicated worker thread that:
// 1. Receives raw audio from Layer 1 input capture
// 2. Resamples to maximum sample rate (e.g., 48kHz)
// 3. Applies per-input effects (EQ, compressor, etc.)
// 4. Sends processed audio to Layer 3 mixing

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Notify};
use tracing::{error, info, warn};

use super::queue_types::ProcessedAudioSamples;
use crate::audio::effects::AudioEffectsChain;
use crate::audio::mixer::sample_rate_converter::RubatoSRC;

/// Input processing worker for a specific device
pub struct InputWorker {
    pub device_id: String,
    pub device_sample_rate: u32, // Original device sample rate
    target_sample_rate: u32, // Max sample rate for mixing (e.g., 48kHz)
    channels: u16,

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
    /// Create a new input processing worker that reads directly from RTRB
    pub fn new_with_rtrb(
        device_id: String,
        device_sample_rate: u32,
        target_sample_rate: u32,
        channels: u16,
        rtrb_consumer: rtrb::Consumer<f32>,
        input_notifier: Arc<Notify>,
        processed_output_tx: mpsc::UnboundedSender<ProcessedAudioSamples>,
    ) -> Self {
        info!("🎤 INPUT_WORKER: Creating RTRB-based worker for device '{}' ({} Hz → {} Hz, {} channels)",
              device_id, device_sample_rate, target_sample_rate, channels);

        Self {
            device_id,
            device_sample_rate,
            target_sample_rate,
            channels,
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

    /// Start the input processing worker thread
    pub fn start(&mut self) -> Result<()> {
        let device_id = self.device_id.clone();
        let device_sample_rate = self.device_sample_rate;
        let target_sample_rate = self.target_sample_rate;
        let channels = self.channels;

        // Clone shared resources for the worker thread
        let rtrb_consumer = self.rtrb_consumer.clone();
        let input_notifier = self.input_notifier.clone();
        let processed_output_tx = self.processed_output_tx.clone();

        // Create new effects chain for worker thread (AudioEffectsChain doesn't implement Clone)
        let mut effects_chain = AudioEffectsChain::new(target_sample_rate);

        // Spawn dedicated worker thread that waits for RTRB notifications
        let worker_handle = tokio::spawn(async move {
            let mut resampler: Option<RubatoSRC> = None;
            let mut samples_processed = 0u64;

            info!(
                "🚀 INPUT_WORKER: Started RTRB notification-driven thread for device '{}'",
                device_id
            );

            // **NOTIFICATION-DRIVEN PROCESSING**: Wait for input data notifications
            loop {
                // Wait for notification that input data is available
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

                    // Read all available samples
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

                // Step 1: Resample to target sample rate if needed
                let resampled_samples =
                    if (device_sample_rate as f32 - target_sample_rate as f32).abs() > 1.0 {
                        // Create resampler if needed (persistent across calls)
                        if resampler.is_none() {
                            match RubatoSRC::new_fast(
                                device_sample_rate as f32,
                                target_sample_rate as f32,
                            ) {
                                Ok(new_resampler) => {
                                    info!(
                                        "🚀 INPUT_WORKER: Created FAST resampler for {} ({} Hz → {} Hz)",
                                        device_id, device_sample_rate, target_sample_rate
                                    );
                                    resampler = Some(new_resampler);
                                }
                                Err(e) => {
                                    error!(
                                        "❌ INPUT_WORKER: Failed to create resampler for {}: {}",
                                        device_id, e
                                    );
                                    // No resampler created - will use original samples below
                                }
                            };
                        }

                        // Resample using persistent resampler
                        if let Some(ref mut resampler) = resampler {
                            resampler.convert(&samples)
                        } else {
                            samples.clone()
                        }
                    } else {
                        // No resampling needed
                        samples.clone()
                    };

                // Step 2: Apply per-input effects (EQ, compressor, etc.)
                let mut effects_processed = resampled_samples;
                effects_chain.process(&mut effects_processed);

                // Step 3: Send processed audio to mixing layer
                let processed_audio = ProcessedAudioSamples {
                    device_id: device_id.clone(),
                    samples: effects_processed,
                    channels,
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
                        "🔄 RESAMPLE_AND_EFFECTS_INPUT_WORKER: (2nd layer) {} processed {} samples in {}μs (batch #{})",
                        device_id,
                        original_samples_len,
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
        self.target_sample_rate = target_mix_rate;
        self.update_effects(AudioEffectsChain::new(target_mix_rate));
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
