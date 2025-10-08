// Layer 3: Mixing Layer
//
// Single-threaded mixer that:
// 1. Receives processed audio from all Layer 2 input workers
// 2. Mixes/sums all input streams together
// 3. Applies master effects and gain
// 4. Sends mixed audio to all Layer 4 output workers

use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use super::temporal_sync_buffer::TemporalSyncBuffer;
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::stream_management::virtual_mixer::VirtualMixer;
use crate::audio::VUChannelService;
use colored::*;

/// Command for dynamically managing running MixingLayer
pub enum MixingLayerCommand {
    AddInputStream {
        device_id: String,
        consumer: Arc<Mutex<rtrb::Consumer<f32>>>,
        queue_tracker: AtomicQueueTracker,
    },
    RemoveInputStream {
        device_id: String,
    },
    AddOutputProducer {
        device_id: String,
        producer: Arc<Mutex<rtrb::Producer<f32>>>,
        queue_tracker: AtomicQueueTracker,
    },
}

/// Mixing layer that combines all processed input streams
pub struct MixingLayer {
    // Input: RTRB consumers from Layer 2 input workers
    input_rtrb_consumers: HashMap<String, Arc<Mutex<rtrb::Consumer<f32>>>>,

    // Queue trackers for monitoring consumer-side reads (one per input device)
    input_queue_trackers: HashMap<String, AtomicQueueTracker>,

    // Output: RTRB producers to Layer 4 output workers
    output_rtrb_producers: HashMap<String, Arc<Mutex<rtrb::Producer<f32>>>>,

    // Queue trackers for monitoring producer-side writes (one per output device)
    output_queue_trackers: HashMap<String, AtomicQueueTracker>,

    // Command channel for dynamic input stream management
    command_tx: mpsc::UnboundedSender<MixingLayerCommand>,

    // Configuration
    target_sample_rate: Arc<AtomicU32>, // Use AtomicU32 for thread-safe dynamic updates
    master_gain: Arc<AtomicU32>,        // Use AtomicU32 to store f32 bits for thread-safe sharing

    // Worker thread
    worker_handle: Option<tokio::task::JoinHandle<()>>,

    // Performance tracking
    mix_cycles: u64,
    samples_mixed: u64,
}

impl MixingLayer {
    /// Get the current sample rate
    fn get_sample_rate(&self) -> u32 {
        self.target_sample_rate.load(Ordering::Relaxed)
    }
    /// Create new mixing layer with dynamic sample rate detection
    pub fn new() -> Self {
        let (command_tx, _command_rx) = mpsc::unbounded_channel();

        Self {
            input_rtrb_consumers: HashMap::new(),
            input_queue_trackers: HashMap::new(),
            output_rtrb_producers: HashMap::new(),
            output_queue_trackers: HashMap::new(),
            command_tx,
            target_sample_rate: Arc::new(AtomicU32::new(0)),
            master_gain: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            worker_handle: None,
            mix_cycles: 0,
            samples_mixed: 0,
        }
    }

    pub fn add_input_consumer(
        &mut self,
        device_id: String,
        consumer: Arc<Mutex<rtrb::Consumer<f32>>>,
        queue_tracker: AtomicQueueTracker,
    ) {
        if self.worker_handle.is_some() {
            let cmd = MixingLayerCommand::AddInputStream {
                device_id: device_id.clone(),
                consumer,
                queue_tracker,
            };
            if let Err(_) = self.command_tx.send(cmd) {
                warn!(
                    "⚠️ {}: Failed to send add input consumer command for '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            } else {
                info!(
                    "🎛️ {}: Sent add input consumer command for device '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            }
        } else {
            self.input_rtrb_consumers
                .insert(device_id.clone(), consumer);
            self.input_queue_trackers
                .insert(device_id.clone(), queue_tracker);
            info!(
                "🎛️ {}: Queued input consumer for device '{}'",
                "MIXING_LAYER".on_green().white(),
                device_id
            );
        }
    }

    /// Remove an input RTRB consumer (stops receiving audio from a device)
    pub fn remove_input_consumer(&mut self, device_id: String) {
        if self.worker_handle.is_some() {
            let cmd = MixingLayerCommand::RemoveInputStream {
                device_id: device_id.clone(),
            };
            if let Err(_) = self.command_tx.send(cmd) {
                warn!(
                    "⚠️ {}: Failed to send remove input consumer command for '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            } else {
                info!(
                    "🗑️ {}: Sent remove input consumer command for device '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            }
        } else {
            self.input_rtrb_consumers.remove(&device_id);
            self.input_queue_trackers.remove(&device_id);
            info!(
                "🗑️ {}: Removed input consumer for device '{}' (not yet started)",
                "MIXING_LAYER".on_green().white(),
                device_id
            );
        }
    }

    /// Add an output RTRB producer (writes mixed audio directly to output workers)
    pub fn add_output_producer(
        &mut self,
        device_id: String,
        producer: Arc<Mutex<rtrb::Producer<f32>>>,
        queue_tracker: AtomicQueueTracker,
    ) {
        if self.worker_handle.is_some() {
            // MixingLayer is already running - send command to worker thread
            let cmd = MixingLayerCommand::AddOutputProducer {
                device_id: device_id.clone(),
                producer,
                queue_tracker,
            };
            if let Err(_) = self.command_tx.send(cmd) {
                warn!(
                    "⚠️ {}: Failed to send add output producer command for '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            } else {
                info!(
                    "🔊 {}: Sent add output producer command for device '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            }
        } else {
            // MixingLayer not started yet - add to local storage
            self.output_rtrb_producers
                .insert(device_id.clone(), producer);
            self.output_queue_trackers
                .insert(device_id.clone(), queue_tracker);
            info!(
                "🔊 {}: Queued output producer for device '{}' (total: {})",
                "MIXING_LAYER".on_green().white(),
                device_id,
                self.output_rtrb_producers.len()
            );
        }
    }

    /// Start the mixing processing thread
    pub fn start(
        &mut self,
        vu_channel: Option<tauri::ipc::Channel<crate::audio::VUChannelData>>,
    ) -> Result<()> {
        // No-op if no sample rate is set (no devices added yet)
        let current_sample_rate = self.target_sample_rate.load(Ordering::Relaxed);
        if current_sample_rate == 0 {
            info!(
                "🎛️ {}: No sample rate set - no devices added yet, skipping start",
                "MIXING_LAYER".on_green().white(),
            );
            return Ok(());
        }

        let target_sample_rate = self.target_sample_rate.clone();
        let master_gain = self.master_gain.clone();

        // Create command channel for this run
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        self.command_tx = command_tx;

        // Take ownership of RTRB consumers and queue trackers for the worker thread
        let mut input_rtrb_consumers = std::mem::take(&mut self.input_rtrb_consumers);
        let mut input_queue_trackers = std::mem::take(&mut self.input_queue_trackers);
        let mut output_rtrb_producers = std::mem::take(&mut self.output_rtrb_producers);
        let mut output_queue_trackers = std::mem::take(&mut self.output_queue_trackers);
        let master_vu_service = vu_channel.map(|channel| {
            info!(
                "{}: VU channel enabled for master output",
                "VU_SETUP".on_green().white()
            );
            VUChannelService::new(channel, current_sample_rate, 1, 60)
        });

        // Spawn mixing worker thread
        let worker_handle = tokio::spawn(async move {
            info!(
                "🚀 {}: Started mixing thread (inputs: {}, outputs: {})",
                "MIXING_LAYER".on_green().white(),
                input_rtrb_consumers.len(),
                output_rtrb_producers.len()
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

                // Handle commands (add/remove input/output streams dynamically)
                let command_start = std::time::Instant::now();
                while let Ok(cmd) = command_rx.try_recv() {
                    match cmd {
                        MixingLayerCommand::AddInputStream {
                            device_id,
                            consumer,
                            queue_tracker,
                        } => {
                            input_rtrb_consumers.insert(device_id.clone(), consumer);
                            input_queue_trackers.insert(device_id.clone(), queue_tracker);
                            info!(
                                "🎛️ MIXING_LAYER_WORKER: Added input consumer for device '{}'",
                                device_id
                            );
                        }
                        MixingLayerCommand::RemoveInputStream { device_id } => {
                            input_rtrb_consumers.remove(&device_id);
                            input_queue_trackers.remove(&device_id);
                            temporal_buffer.remove_device(&device_id);
                            info!(
                                "🗑️ MIXING_LAYER_WORKER: Removed input consumer for device '{}' (remaining: {})",
                                device_id,
                                input_rtrb_consumers.len()
                            );
                        }
                        MixingLayerCommand::AddOutputProducer {
                            device_id,
                            producer,
                            queue_tracker,
                        } => {
                            output_rtrb_producers.insert(device_id.clone(), producer);
                            output_queue_trackers.insert(device_id.clone(), queue_tracker);
                            info!(
                                "🔊 MIXING_LAYER_WORKER: Added output producer for device '{}' (total: {})",
                                device_id,
                                output_rtrb_producers.len()
                            );
                        }
                    }
                }
                let command_duration = command_start.elapsed();

                // **TEMPORAL SYNC STEP 1**: Collect samples from RTRB and add to temporal buffer
                let collection_start = std::time::Instant::now();
                for (device_id, consumer) in input_rtrb_consumers.iter() {
                    let mut consumer_lock = consumer.lock().await;
                    let available = consumer_lock.slots();

                    if available > 0 {
                        let mut samples = Vec::with_capacity(available);
                        let mut samples_read = 0;

                        while let Ok(sample) = consumer_lock.pop() {
                            samples.push(sample);
                            samples_read += 1;
                        }

                        if !samples.is_empty() {
                            // **DIAGNOSTIC**: Log RTRB collection details
                            static COLLECTION_LOG_COUNT: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let coll_count = COLLECTION_LOG_COUNT
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if coll_count < 20 || coll_count % 500 == 0 {
                                info!(
                                    "🔄 {}: Device '{}' collected {} samples from RTRB (available: {})",
                                    "MIXING_COLLECT".cyan(),
                                    device_id,
                                    samples.len(),
                                    available
                                );
                            }

                            // Construct ProcessedAudioSamples from raw RTRB data
                            let processed_audio = super::queue_types::ProcessedAudioSamples {
                                device_id: device_id.clone(),
                                samples,
                                channels: 2, // All inputs are converted to stereo by InputWorker
                                timestamp: std::time::Instant::now(),
                                effects_applied: true, // InputWorker applies effects
                            };

                            let sample_count = processed_audio.samples.len();
                            temporal_buffer.add_samples(device_id.clone(), processed_audio);

                            // Record samples read for queue tracking
                            if let Some(tracker) = input_queue_trackers.get(device_id) {
                                tracker.record_samples_read(sample_count);
                            }

                            mixed_something = true;
                        }
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

                    // **DIAGNOSTIC**: Log input sample counts before mixing
                    static PREMIX_LOG_COUNT: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let premix_count =
                        PREMIX_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if premix_count < 20 || premix_count % 500 == 0 {
                        let sample_details: Vec<String> = input_samples_for_mixer
                            .iter()
                            .map(|(id, samples)| format!("{}: {} samples", id, samples.len()))
                            .collect();
                        info!(
                            "🎛️ {}: Preparing to mix {} inputs: [{}]",
                            "PRE_MIX".magenta(),
                            active_inputs,
                            sample_details.join(", ")
                        );
                    }

                    if !input_samples_for_mixer.is_empty() {
                        let mix_start = std::time::Instant::now();
                        let mixed_samples =
                            VirtualMixer::mix_input_samples_ref(&input_samples_for_mixer);
                        let mix_duration = mix_start.elapsed();

                        // Apply master gain to the mixed samples
                        let gain_start = std::time::Instant::now();
                        let mut final_samples = mixed_samples;
                        let current_gain = f32::from_bits(master_gain.load(Ordering::Relaxed));
                        for sample in final_samples.iter_mut() {
                            *sample *= current_gain;
                        }
                        let gain_duration = gain_start.elapsed();

                        if let Some(ref vu_service) = master_vu_service {
                            vu_service.queue_master_audio(&final_samples);
                        }

                        let samples_count = final_samples.len(); // Get count before moving

                        // Step 3: Write mixed audio directly to all output RTRB queues
                        let broadcast_start = std::time::Instant::now();

                        for (device_id, producer) in output_rtrb_producers.iter() {
                            let mut producer_lock = producer.lock().await;
                            let mut samples_written = 0;
                            let mut remaining = final_samples.as_slice();

                            // Write samples to RTRB queue using the same pattern as audio_worker
                            while !remaining.is_empty() && samples_written < final_samples.len() {
                                let chunk_size = remaining.len().min(producer_lock.slots());
                                if chunk_size == 0 {
                                    // warn!(
                                    //     "⚠️ {}: Output '{}' RTRB queue full, dropping {} remaining samples",
                                    //     "MIXING_LAYER".on_green().white(),
                                    //     device_id,
                                    //     remaining.len()
                                    // );
                                    break;
                                }

                                let chunk = &remaining[..chunk_size];
                                for &sample in chunk {
                                    if producer_lock.push(sample).is_err() {
                                        break;
                                    }
                                    samples_written += 1;
                                }
                                remaining = &remaining[chunk_size..];
                            }

                            // Record samples written for queue tracking
                            if let Some(tracker) = output_queue_trackers.get(device_id) {
                                tracker.record_samples_written(samples_written);
                            }

                            if samples_written < final_samples.len() {
                                // warn!(
                                //     "⚠️ {}: Partial write to output '{}': {} of {} samples",
                                //     "MIXING_LAYER".on_green().white(),
                                //     device_id,
                                //     samples_written,
                                //     final_samples.len()
                                // );
                            }
                        }
                        let broadcast_duration = broadcast_start.elapsed();

                        mix_cycles += 1;

                        let total_mixing_duration = mixing_start.elapsed();

                        // Rate-limited logging (only when we actually mixed something)
                        if mix_cycles <= 5 || mix_cycles % 1000 == 0 {
                            info!("🎵 {}: TEMPORAL SYNC mixed {} inputs ({} samples) and wrote to {} outputs (cycle #{}, sync took {}μs, total {}μs)",
                                  "MIXING_LAYER_TEMPORAL".on_green().white(),
                                  active_inputs, samples_count, output_rtrb_producers.len(), mix_cycles, sync_duration.as_micros(), total_mixing_duration.as_micros());
                        }

                        // Performance monitoring with detailed breakdown (only when we actually mixed something)
                        if total_mixing_duration.as_micros() > 1000 {
                            warn!(
                                "⏱️ {}: Slow mixing cycle: total {}μs (prep: {}μs, mix: {}μs, gain: {}μs, broadcast: {}μs)",
                                "MIXING_LAYER_SLOW".on_green().white(),
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
                        "TEMPORAL_CYCLE_BREAKDOWN".on_green().white(),
                        cycle_duration.as_micros(),
                        command_duration.as_micros(),
                        collection_duration.as_micros(),
                        sync_duration.as_micros(),
                        mixing_duration.as_micros()
                    );
                }

                // Small yield to prevent busy-waiting
                if !mixed_something {
                    tokio::time::sleep(std::time::Duration::from_micros(25)).await;
                }
            }
        });

        self.worker_handle = Some(worker_handle);
        info!(
            "✅ {}: Started mixing worker thread",
            "MIXING_LAYER".on_green().white(),
        );

        Ok(())
    }

    /// Stop the mixing layer
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();

            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => info!(
                    "✅ {}: Shut down gracefully",
                    "MIXING_LAYER".on_green().white()
                ),
                Err(_) => warn!(
                    "⚠️ {}: Force-terminated after timeout",
                    "MIXING_LAYER".on_green().white()
                ),
            }
        }

        Ok(())
    }

    pub fn set_master_gain(&mut self, gain: f32) {
        self.master_gain.store(gain.to_bits(), Ordering::Relaxed);
        info!(
            "🎚️ {}: Set master gain to {:.2}",
            "MIXING_LAYER".on_green().white(),
            gain
        );
    }

    pub fn update_target_sample_rate(&mut self, new_sample_rate: u32) {
        self.target_sample_rate
            .store(new_sample_rate, Ordering::Relaxed);
        info!(
            "🎛️ {}: Updated target sample rate to {} Hz",
            "MIXING_LAYER".on_green().white(),
            new_sample_rate
        );
    }

    pub fn get_stats(&self) -> MixingLayerStats {
        MixingLayerStats {
            mix_cycles: self.mix_cycles,
            samples_mixed: self.samples_mixed,
            input_streams: self.input_rtrb_consumers.len(),
            output_streams: self.output_rtrb_producers.len(),
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
