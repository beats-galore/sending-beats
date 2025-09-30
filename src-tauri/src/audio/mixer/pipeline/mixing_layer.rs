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
use super::temporal_sync_buffer::TemporalSyncBuffer;
use crate::audio::mixer::stream_management::virtual_mixer::VirtualMixer;
use crate::audio::VUChannelService;
use colored::*;

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
    target_sample_rate: Option<u32>,
    master_gain: f32,

    // Worker thread
    worker_handle: Option<tokio::task::JoinHandle<()>>,

    // Performance tracking
    mix_cycles: u64,
    samples_mixed: u64,
}

impl MixingLayer {
    /// Get the current sample rate, panics if not set (must have devices added first)
    fn get_sample_rate(&self) -> u32 {
        self.target_sample_rate
            .expect("MixingLayer sample rate not set - no devices have been added yet")
    }
    /// Create new mixing layer with dynamic sample rate detection
    pub fn new() -> Self {
        let (command_tx, _command_rx) = mpsc::unbounded_channel();

        Self {
            processed_input_receivers: HashMap::new(),
            mixed_output_senders: Vec::new(),
            command_tx,
            target_sample_rate: None,
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
    pub fn start(
        &mut self,
        vu_channel: Option<tauri::ipc::Channel<crate::audio::VUChannelData>>,
    ) -> Result<()> {
        // No-op if no sample rate is set (no devices added yet)
        let target_sample_rate = match self.target_sample_rate {
            Some(rate) => rate,
            None => {
                info!("🎛️ MIXING_LAYER: No sample rate set - no devices added yet, skipping start");
                return Ok(());
            }
        };
        let master_gain = self.master_gain;

        // Create command channel for this run
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        self.command_tx = command_tx;

        // Take ownership of receivers for the worker thread
        let mut processed_input_receivers = std::mem::take(&mut self.processed_input_receivers);
        let mut mixed_output_senders = self.mixed_output_senders.clone();
        let master_vu_service = vu_channel.map(|channel| {
            info!(
                "{}: VU channel enabled for master output",
                "VU_SETUP".on_green().white()
            );
            VUChannelService::new(channel, target_sample_rate, 1, 60)
        });

        // Spawn mixing worker thread
        let worker_handle = tokio::spawn(async move {
            info!(
                "🚀 MIXING_LAYER: Started mixing thread (inputs: {}, outputs: {})",
                processed_input_receivers.len(),
                mixed_output_senders.len()
            );

            let mut mix_cycles = 0u64;
            let mut cleanup_cycle_count = 0u64;

            // **TEMPORAL SYNCHRONIZATION**: Initialize temporal sync buffer
            // 25ms sync window allows for typical hardware callback timing variations
            let mut temporal_buffer = TemporalSyncBuffer::new(25, 10); // 25ms window, max 10 samples per device

            // **PERFORMANCE FIX**: Pre-allocate reusable vectors outside the loop
            let mut input_samples_for_mixer: Vec<(String, &[f32])> = Vec::with_capacity(8);

            loop {
                let cycle_start = std::time::Instant::now();
                let mut mixed_something = false;

                // Handle commands (add new input/output streams dynamically)
                let command_start = std::time::Instant::now();
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
                let command_duration = command_start.elapsed();

                // **TEMPORAL SYNC STEP 1**: Collect samples and add to temporal buffer
                let collection_start = std::time::Instant::now();
                for (device_id, receiver) in processed_input_receivers.iter_mut() {
                    // Non-blocking receive - get whatever samples are available
                    while let Ok(processed_audio) = receiver.try_recv() {
                        temporal_buffer.add_samples(device_id.clone(), processed_audio);
                        mixed_something = true;
                    }
                }
                let collection_duration = collection_start.elapsed();

                // **TEMPORAL SYNC STEP 2**: Extract synchronized samples from buffer
                let sync_start = std::time::Instant::now();
                let synchronized_samples = temporal_buffer.extract_synchronized_samples();
                let sync_duration = sync_start.elapsed();

                // Periodic cleanup to prevent memory bloat (every 1000 cycles ≈ 20 seconds)
                cleanup_cycle_count += 1;
                if cleanup_cycle_count % 1000 == 0 {
                    temporal_buffer.cleanup_old_samples();
                }

                // **TEMPORAL SYNC STEP 3**: Mix synchronized samples if we have any
                let mixing_duration = if !synchronized_samples.is_empty() {
                    let mixing_start = std::time::Instant::now();

                    // **TEMPORAL SYNC FIX**: Use synchronized samples instead of raw available samples
                    let prep_start = std::time::Instant::now();
                    let input_samples_for_mixer: Vec<(String, &[f32])> = synchronized_samples
                        .iter()
                        .map(|(device_id, processed_audio)| {
                            (device_id.clone(), processed_audio.samples.as_slice())
                        })
                        .collect();
                    let prep_duration = prep_start.elapsed();

                    let active_inputs = input_samples_for_mixer.len();

                    if !input_samples_for_mixer.is_empty() {
                        let mix_start = std::time::Instant::now();
                        let mixed_samples =
                            VirtualMixer::mix_input_samples_ref(&input_samples_for_mixer);
                        let mix_duration = mix_start.elapsed();

                        // Apply master gain to the mixed samples
                        let gain_start = std::time::Instant::now();
                        let mut final_samples = mixed_samples;
                        for sample in final_samples.iter_mut() {
                            *sample *= master_gain;
                        }
                        let gain_duration = gain_start.elapsed();

                        if let Some(ref vu_service) = master_vu_service {
                            vu_service.queue_master_audio(&final_samples);
                        }

                        let samples_count = final_samples.len(); // Get count before moving

                        // Step 3: Broadcast mixed audio to all output workers
                        let broadcast_start = std::time::Instant::now();
                        let mixed_audio = MixedAudioSamples {
                            samples: final_samples,
                            sample_rate: target_sample_rate,
                            timestamp: std::time::Instant::now(),
                            input_count: active_inputs,
                        };

                        // Send to all output senders
                        // **PERFORMANCE NOTE**: Each output still requires a clone due to queue_types structure
                        // Future optimization could use Arc<Vec<f32>> in queue_types to eliminate this
                        for sender in mixed_output_senders.iter() {
                            if let Err(_) = sender.send(mixed_audio.clone()) {
                                warn!("⚠️ MIXING_LAYER: Failed to send to output worker (may be shut down)");
                            }
                        }
                        let broadcast_duration = broadcast_start.elapsed();

                        mix_cycles += 1;

                        let total_mixing_duration = mixing_start.elapsed();

                        // Rate-limited logging (only when we actually mixed something)
                        if mix_cycles <= 5 || mix_cycles % 1000 == 0 {
                            info!("🎵 {}: TEMPORAL SYNC mixed {} inputs ({} samples) and sent to {} outputs (cycle #{}, sync took {}μs, total {}μs)",
                                  "MIXING_LAYER_TEMPORAL".cyan(),
                                  active_inputs, samples_count, mixed_output_senders.len(), mix_cycles, sync_duration.as_micros(), total_mixing_duration.as_micros());
                        }

                        // Performance monitoring with detailed breakdown (only when we actually mixed something)
                        if total_mixing_duration.as_micros() > 1000 {
                            warn!(
                                "⏱️ {}: Slow mixing cycle: total {}μs (prep: {}μs, mix: {}μs, gain: {}μs, broadcast: {}μs)",
                                "MIXING_LAYER_SLOW".yellow(),
                                total_mixing_duration.as_micros(),
                                prep_duration.as_micros(),
                                mix_duration.as_micros(),
                                gain_duration.as_micros(),
                                broadcast_duration.as_micros()
                            );
                        }

                        total_mixing_duration
                    } else {
                        std::time::Duration::ZERO
                    }
                } else {
                    std::time::Duration::ZERO
                };

                let cycle_duration = cycle_start.elapsed();

                // Log full cycle breakdown for very slow cycles
                if cycle_duration.as_micros() > 2000 {
                    warn!(
                        "⏱️ {}: Very slow cycle: total {}μs (commands: {}μs, collection: {}μs, sync: {}μs, mixing: {}μs)",
                        "TEMPORAL_CYCLE_BREAKDOWN".yellow(),
                        cycle_duration.as_micros(),
                        command_duration.as_micros(),
                        collection_duration.as_micros(),
                        sync_duration.as_micros(),
                        mixing_duration.as_micros()
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

    pub fn set_master_gain(&mut self, gain: f32) {
        self.master_gain = gain;
        info!("🎚️ MIXING_LAYER: Set master gain to {:.2}", gain);
    }

    pub fn update_target_sample_rate(&mut self, new_sample_rate: u32) {
        self.target_sample_rate = Some(new_sample_rate);
        info!(
            "🎛️ MIXING_LAYER: Updated target sample rate to {} Hz",
            new_sample_rate
        );
    }

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
