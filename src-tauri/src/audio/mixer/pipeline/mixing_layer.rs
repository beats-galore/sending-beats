// Layer 3: Mixing Layer
//
// Single-threaded mixer that:
// 1. Receives processed audio from all Layer 2 input workers
// 2. Mixes/sums all input streams together
// 3. Applies master effects and gain
// 4. Sends mixed audio to all Layer 4 output workers

use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::queue_types::{MixedAudioSamples, ProcessedAudioSamples};
use crate::audio::mixer::stream_management::virtual_mixer::VirtualMixer;

/// Command for dynamically managing running MixingLayer
pub enum MixingLayerCommand {
    AddInputStream {
        device_id: String,
        receiver: mpsc::UnboundedReceiver<ProcessedAudioSamples>,
    },
    AddOutputSender {
        sender: mpsc::UnboundedSender<MixedAudioSamples>,
    },
}

/// Mixing layer that combines all processed input streams
pub struct MixingLayer {
    // Input: Processed streams from Layer 2
    processed_input_receivers: HashMap<String, mpsc::UnboundedReceiver<ProcessedAudioSamples>>,

    // Output: Mixed stream to Layer 4
    mixed_output_senders: Vec<mpsc::UnboundedSender<MixedAudioSamples>>, // Broadcast to all output devices

    // Command channel for dynamic input stream management
    command_tx: mpsc::UnboundedSender<MixingLayerCommand>,

    // Configuration
    target_sample_rate: u32,
    master_gain: f32,

    // Worker thread
    worker_handle: Option<tokio::task::JoinHandle<()>>,

    // Performance tracking
    mix_cycles: u64,
    samples_mixed: u64,
}

impl MixingLayer {
    /// Create new mixing layer
    pub fn new(target_sample_rate: u32) -> Self {
        let (command_tx, _command_rx) = mpsc::unbounded_channel();

        Self {
            processed_input_receivers: HashMap::new(),
            mixed_output_senders: Vec::new(),
            command_tx,
            target_sample_rate,
            master_gain: 1.0,
            worker_handle: None,
            mix_cycles: 0,
            samples_mixed: 0,
        }
    }

    /// Add an input stream from an input worker (dynamically)
    pub fn add_input_stream(
        &mut self,
        device_id: String,
        receiver: mpsc::UnboundedReceiver<ProcessedAudioSamples>,
    ) {
        if self.worker_handle.is_some() {
            // MixingLayer is already running - send command to worker thread
            let cmd = MixingLayerCommand::AddInputStream {
                device_id: device_id.clone(),
                receiver,
            };
            if let Err(_) = self.command_tx.send(cmd) {
                warn!(
                    "⚠️ MIXING_LAYER: Failed to send add input stream command for '{}'",
                    device_id
                );
            } else {
                info!(
                    "🎛️ MIXING_LAYER: Sent add input stream command for device '{}'",
                    device_id
                );
            }
        } else {
            // MixingLayer not started yet - add to local storage
            self.processed_input_receivers
                .insert(device_id.clone(), receiver);
            info!(
                "🎛️ MIXING_LAYER: Queued input stream for device '{}'",
                device_id
            );
        }
    }

    /// Add an output sender (broadcasts mixed audio to output workers)
    pub fn add_output_sender(&mut self, sender: mpsc::UnboundedSender<MixedAudioSamples>) {
        if self.worker_handle.is_some() {
            // MixingLayer is already running - send command to worker thread
            let cmd = MixingLayerCommand::AddOutputSender { sender };
            if let Err(_) = self.command_tx.send(cmd) {
                warn!("⚠️ MIXING_LAYER: Failed to send add output sender command");
            } else {
                info!("🔊 MIXING_LAYER: Sent add output sender command");
            }
        } else {
            // MixingLayer not started yet - add to local storage
            self.mixed_output_senders.push(sender);
            info!(
                "🔊 MIXING_LAYER: Queued output sender (total: {})",
                self.mixed_output_senders.len()
            );
        }
    }

    /// Start the mixing processing thread
    pub fn start(&mut self) -> Result<()> {
        let target_sample_rate = self.target_sample_rate;
        let master_gain = self.master_gain;

        // Create command channel for this run
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        self.command_tx = command_tx;

        // Take ownership of receivers for the worker thread
        let mut processed_input_receivers = std::mem::take(&mut self.processed_input_receivers);
        let mut mixed_output_senders = self.mixed_output_senders.clone();

        // Spawn mixing worker thread
        let worker_handle = tokio::spawn(async move {
            info!(
                "🚀 MIXING_LAYER: Started mixing thread (inputs: {}, outputs: {})",
                processed_input_receivers.len(),
                mixed_output_senders.len()
            );

            let mut mix_cycles = 0u64;
            let mut available_samples = HashMap::new();

            loop {
                let cycle_start = std::time::Instant::now();
                let mut mixed_something = false;

                // Handle commands (add new input/output streams dynamically)
                while let Ok(cmd) = command_rx.try_recv() {
                    match cmd {
                        MixingLayerCommand::AddInputStream {
                            device_id,
                            receiver,
                        } => {
                            processed_input_receivers.insert(device_id.clone(), receiver);
                            info!(
                                "🎛️ MIXING_LAYER_WORKER: Added input stream for device '{}'",
                                device_id
                            );
                        }
                        MixingLayerCommand::AddOutputSender { sender } => {
                            mixed_output_senders.push(sender);
                            info!(
                                "🔊 MIXING_LAYER_WORKER: Added output sender (total: {})",
                                mixed_output_senders.len()
                            );
                        }
                    }
                }

                // Step 1: Collect available samples from all input streams
                available_samples.clear();
                for (device_id, receiver) in processed_input_receivers.iter_mut() {
                    // Non-blocking receive - get whatever samples are available
                    while let Ok(processed_audio) = receiver.try_recv() {
                        available_samples.insert(device_id.clone(), processed_audio);
                        mixed_something = true;
                    }
                }

                // Step 2: Mix available samples if we have any
                if mixed_something && !available_samples.is_empty() {
                    // Convert ProcessedAudioSamples to the format expected by VirtualMixer
                    let input_samples_for_mixer: Vec<(String, Vec<f32>)> = available_samples
                        .iter()
                        .map(|(device_id, processed_audio)| {
                            (device_id.clone(), processed_audio.samples.clone())
                        })
                        .collect();

                    let active_inputs = input_samples_for_mixer.len();

                    if !input_samples_for_mixer.is_empty() {
                        // Use VirtualMixer's professional mixing algorithm
                        let mixed_samples =
                            VirtualMixer::mix_input_samples(input_samples_for_mixer);

                        // Apply master gain to the professionally mixed samples
                        let mut final_samples = mixed_samples;
                        for sample in final_samples.iter_mut() {
                            *sample *= master_gain;
                        }

                        let samples_count = final_samples.len(); // Get count before moving

                        // Step 3: Broadcast mixed audio to all output workers
                        let mixed_audio = MixedAudioSamples {
                            samples: final_samples,
                            sample_rate: target_sample_rate,
                            timestamp: std::time::Instant::now(),
                            input_count: active_inputs,
                        };

                        // Send to all output senders
                        for sender in mixed_output_senders.iter() {
                            if let Err(_) = sender.send(mixed_audio.clone()) {
                                warn!("⚠️ MIXING_LAYER: Failed to send to output worker (may be shut down)");
                            }
                        }

                        mix_cycles += 1;

                        // Rate-limited logging
                        if mix_cycles <= 5 || mix_cycles % 1000 == 0 {
                            info!("🎵 MIXING_LAYER_WORKER (3rd layer): VirtualMixer mixed {} inputs ({} samples) and sent to {} outputs (cycle #{})",
                                  active_inputs, samples_count, mixed_output_senders.len(), mix_cycles);
                        }
                    }
                }

                let cycle_duration = cycle_start.elapsed();

                // Performance monitoring
                if cycle_duration.as_micros() > 1000 {
                    warn!(
                        "⏱️ MIXING_LAYER: Slow mixing cycle: {}μs",
                        cycle_duration.as_micros()
                    );
                }

                // Small yield to prevent busy-waiting
                if !mixed_something {
                    tokio::time::sleep(std::time::Duration::from_micros(100)).await;
                }
            }
        });

        self.worker_handle = Some(worker_handle);
        info!("✅ MIXING_LAYER: Started mixing worker thread");

        Ok(())
    }

    /// Stop the mixing layer
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();

            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => info!("✅ MIXING_LAYER: Shut down gracefully"),
                Err(_) => warn!("⚠️ MIXING_LAYER: Force-terminated after timeout"),
            }
        }

        Ok(())
    }

    /// Update master gain
    pub fn set_master_gain(&mut self, gain: f32) {
        self.master_gain = gain;
        info!("🎚️ MIXING_LAYER: Set master gain to {:.2}", gain);
    }

    /// Update target sample rate when devices are added/removed
    pub fn update_target_sample_rate(&mut self, new_sample_rate: u32) {
        self.target_sample_rate = new_sample_rate;
        info!(
            "🎛️ MIXING_LAYER: Updated target sample rate to {} Hz",
            new_sample_rate
        );
    }

    /// Get mixing statistics
    pub fn get_stats(&self) -> MixingLayerStats {
        MixingLayerStats {
            mix_cycles: self.mix_cycles,
            samples_mixed: self.samples_mixed,
            input_streams: self.processed_input_receivers.len(),
            output_streams: self.mixed_output_senders.len(),
            is_running: self.worker_handle.is_some(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MixingLayerStats {
    pub mix_cycles: u64,
    pub samples_mixed: u64,
    pub input_streams: usize,
    pub output_streams: usize,
    pub is_running: bool,
}
