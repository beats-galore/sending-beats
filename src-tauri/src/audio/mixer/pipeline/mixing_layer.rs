// Layer 3: Mixing Layer
//
// Single-threaded mixer that:
// 1. Receives processed audio from all Layer 2 input workers
// 2. Mixes/sums all input streams together
// 3. Applies master effects and gain
// 4. Sends mixed audio to all Layer 4 output workers

use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use super::audio_worker::target_downstream_samples;
use super::block_accumulator::BlockAccumulator;
use super::pacing::jitter_cushion_samples;
use crate::audio::mixer::latency_probe::{LatencyProbe, LatencyStage, StageGauge};
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::stream_management::virtual_mixer::VirtualMixer;
use crate::audio::VUChannelService;
use colored::*;

/// Block size before any output device has said what its hardware wants
const DEFAULT_MIX_BLOCK_SAMPLES: usize = 1024;

/// Backlog a device may build before its oldest audio is dropped, in stereo samples
///
/// ~85ms at 48kHz. Deliberately not a multiple of the block size: shrinking the
/// block must not shrink the amount a coarse source is allowed to hold, and a
/// ScreenCaptureKit tap arrives 960 frames at a time whatever the mixer does.
const MAX_BACKLOG_SAMPLES: usize = 8192;

/// Audio a device is watched over before a level it never drained below is shed
///
/// ~2 seconds at 48kHz, which has to comfortably exceed how far apart a source's
/// deliveries can be. ScreenCaptureKit batches several callbacks together and
/// then goes quiet, and a window shorter than one of those gaps would read the
/// quiet part as a standing backlog and start cutting into bursts.
const BACKLOG_WINDOW_SAMPLES: usize = 192_000;

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
    RemoveOutputProducer {
        device_id: String,
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

    /// Stereo samples per mix block, tracking the tightest output's hardware buffer
    mix_block_samples: Arc<AtomicUsize>,

    // Latency accounting for the block accumulator and the mix block itself
    latency_probe: Arc<LatencyProbe>,

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
    pub fn new(latency_probe: Arc<LatencyProbe>) -> Self {
        let (command_tx, _command_rx) = mpsc::unbounded_channel();

        Self {
            input_rtrb_consumers: HashMap::new(),
            input_queue_trackers: HashMap::new(),
            output_rtrb_producers: HashMap::new(),
            output_queue_trackers: HashMap::new(),
            command_tx,
            target_sample_rate: Arc::new(AtomicU32::new(0)),
            master_gain: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            mix_block_samples: Arc::new(AtomicUsize::new(DEFAULT_MIX_BLOCK_SAMPLES)),
            latency_probe,
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

    /// Stop writing the mix to an output device.
    ///
    /// Must be called whenever an output worker goes away: the mixer only
    /// produces once every registered producer can take a full block, so a
    /// producer left behind by a stopped worker stalls the mix permanently.
    pub fn remove_output_producer(&mut self, device_id: String) {
        if self.worker_handle.is_some() {
            let cmd = MixingLayerCommand::RemoveOutputProducer {
                device_id: device_id.clone(),
            };
            if self.command_tx.send(cmd).is_err() {
                warn!(
                    "⚠️ {}: Failed to send remove output producer command for '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            } else {
                info!(
                    "🗑️ {}: Sent remove output producer command for device '{}'",
                    "MIXING_LAYER".on_green().white(),
                    device_id
                );
            }
        } else {
            self.output_rtrb_producers.remove(&device_id);
            self.output_queue_trackers.remove(&device_id);
            info!(
                "🗑️ {}: Removed output producer for device '{}' (not yet started)",
                "MIXING_LAYER".on_green().white(),
                device_id
            );
        }
    }

    /// Start the mixing processing thread
    pub fn start(&mut self, vu_channel: crate::audio::SharedVUChannel) -> Result<()> {
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
        let mix_block_samples = self.mix_block_samples.clone();

        // Create command channel for this run
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        self.command_tx = command_tx;

        // Take ownership of RTRB consumers and queue trackers for the worker thread
        let mut input_rtrb_consumers = std::mem::take(&mut self.input_rtrb_consumers);
        let mut input_queue_trackers = std::mem::take(&mut self.input_queue_trackers);
        let mut output_rtrb_producers = std::mem::take(&mut self.output_rtrb_producers);
        let mut output_queue_trackers = std::mem::take(&mut self.output_queue_trackers);
        // Started unconditionally. The mixing layer starts with the first device,
        // which can be before the frontend has registered a channel, and it is
        // only ever started once — so creating this conditionally left master
        // metering dead for the whole session whenever the ordering went that way.
        info!(
            "{}: VU metering enabled for master output",
            "VU_SETUP".on_green().white()
        );
        let master_vu_service = Some(VUChannelService::new(
            vu_channel,
            current_sample_rate,
            1,
            60,
        ));

        let latency_probe = self.latency_probe.clone();

        // Spawn mixing worker thread
        let worker_handle = tokio::spawn(async move {
            info!(
                "🚀 {}: Started mixing thread (inputs: {}, outputs: {})",
                "MIXING_LAYER".on_green().white(),
                input_rtrb_consumers.len(),
                output_rtrb_producers.len()
            );

            let mut mix_cycles = 0u64;

            // **FIXED CADENCE**: Every cycle consumes the same block from each device
            // so the mix advances at a constant rate regardless of how much any one
            // device happened to deliver.
            //
            // The block tracks the tightest output's hardware buffer. It has to: the
            // mixer pads a device short of a full block with silence rather than
            // waiting, so a block larger than what the hardware delivers per callback
            // punches holes in the audio, and one smaller makes the mixer outrun the
            // output it is paced by.
            let mut block_accumulator = BlockAccumulator::new(
                mix_block_samples.load(Ordering::Relaxed),
                jitter_cushion_samples(current_sample_rate, 2),
                BACKLOG_WINDOW_SAMPLES,
                MAX_BACKLOG_SAMPLES,
            );

            // Resolved once per device: looking a gauge up in the registry takes a
            // lock and allocates, neither of which belongs in the mixing loop.
            let mix_gauge = latency_probe.mix_gauge();
            let mut backlog_gauges: HashMap<String, StageGauge> = HashMap::new();

            loop {
                let cycle_start = std::time::Instant::now();
                let mut produced_block = false;

                // Follow the outputs when they report what their hardware settled on
                let block_samples = mix_block_samples.load(Ordering::Relaxed);
                if block_samples != block_accumulator.block_samples() {
                    info!(
                        "🎛️ {}: Mix block now {} samples ({} frames), was {}",
                        "MIXING_LAYER".on_green().white(),
                        block_samples,
                        block_samples / 2,
                        block_accumulator.block_samples()
                    );
                    block_accumulator.set_block_samples(block_samples);
                    block_accumulator.set_cushion_samples(jitter_cushion_samples(
                        target_sample_rate.load(Ordering::Relaxed),
                        2,
                    ));
                }

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
                            block_accumulator.remove_device(&device_id);
                            backlog_gauges.remove(&device_id);
                            latency_probe.remove_device(&device_id);
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
                        MixingLayerCommand::RemoveOutputProducer { device_id } => {
                            // Dropping this is what keeps the mixer alive. Production
                            // waits until every producer has room for a full block, so
                            // a producer whose worker has stopped never drains and
                            // would hold the mix at a standstill forever.
                            output_rtrb_producers.remove(&device_id);
                            output_queue_trackers.remove(&device_id);
                            latency_probe.remove_device(&device_id);
                            info!(
                                "🗑️ MIXING_LAYER_WORKER: Removed output producer for device '{}' (remaining: {})",
                                device_id,
                                output_rtrb_producers.len()
                            );
                        }
                    }
                }
                let command_duration = command_start.elapsed();

                // **STEP 1**: Collect samples from RTRB and accumulate per device
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

                            // All inputs are already stereo and effected by InputWorker
                            let sample_count = samples.len();
                            block_accumulator.push(device_id, &samples);

                            // Record samples read for queue tracking
                            if let Some(tracker) = input_queue_trackers.get(device_id) {
                                tracker.record_samples_read(sample_count);
                            }
                        }
                    }
                }

                // Read here rather than captured at start so the reported figures
                // follow the mix rate when a device changes it.
                let mix_rate = target_sample_rate.load(Ordering::Relaxed);
                mix_gauge.set_samples(block_samples, 2, mix_rate);

                // Published for every device, not just those that delivered this
                // cycle: a device that went quiet still has whatever it left behind
                // waiting, and that is still delay.
                for device_id in input_rtrb_consumers.keys() {
                    // Looked up rather than `entry`, which would allocate a key
                    // every cycle for devices that are already registered.
                    if !backlog_gauges.contains_key(device_id) {
                        backlog_gauges.insert(
                            device_id.clone(),
                            latency_probe.gauge(device_id, LatencyStage::InputBacklog),
                        );
                    }

                    // Everything reaching the accumulator has been widened to stereo
                    if let Some(gauge) = backlog_gauges.get(device_id) {
                        gauge.set_samples(
                            block_accumulator.backlog_samples(device_id),
                            2,
                            mix_rate,
                        );
                    }
                }

                let collection_duration = collection_start.elapsed();

                // **STEP 2**: Only produce while every output is still short of the
                // amount of audio we want it holding.
                //
                // This is what paces the mixer. Without it the loop free-runs and
                // overproduces, and the surplus is discarded at the output queue,
                // which is audible as crunch. Holding back makes the output
                // hardware's drain rate the mixer's clock.
                //
                // The condition is how much the output is *holding*, not whether it
                // has room: producing on room refills the ring to capacity each time
                // a chunk drains, making the whole ring standing delay.
                let sync_start = std::time::Instant::now();

                let target_queued = target_downstream_samples(block_samples, mix_rate, 2);
                let mut outputs_ready = !output_rtrb_producers.is_empty();
                for (device_id, producer) in output_rtrb_producers.iter() {
                    let producer_lock = producer.lock().await;
                    let free = producer_lock.slots();

                    // Without a tracker there is no capacity to compare against, so
                    // fall back to the room check rather than stalling the mix.
                    let queued = output_queue_trackers
                        .get(device_id)
                        .map_or(0, |tracker| tracker.capacity.saturating_sub(free));

                    if free < block_samples || queued >= target_queued {
                        outputs_ready = false;
                        break;
                    }
                }

                let synchronized_samples = if outputs_ready {
                    block_accumulator.take_block()
                } else {
                    None
                };
                produced_block = synchronized_samples.is_some();
                let sync_duration = sync_start.elapsed();

                // **STEP 3**: Mix the block
                let mixing_duration = if let Some(synchronized_samples) = synchronized_samples {
                    let mixing_start = std::time::Instant::now();

                    // Every block is already exactly block_samples long
                    let prep_start = std::time::Instant::now();

                    let input_samples_for_mixer: Vec<(String, &[f32])> = synchronized_samples
                        .iter()
                        .map(|(device_id, samples)| (device_id.clone(), samples.as_slice()))
                        .collect();
                    let prep_duration = prep_start.elapsed();

                    let active_inputs = input_samples_for_mixer.len();

                    // **DIAGNOSTIC**: Log input sample counts before mixing with detailed chunk info
                    static PREMIX_LOG_COUNT: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let premix_count =
                        PREMIX_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if premix_count < 20 || premix_count % 500 == 0 {
                        // Count chunks per device
                        let mut device_chunks: std::collections::HashMap<String, Vec<usize>> =
                            std::collections::HashMap::new();
                        for (id, samples) in input_samples_for_mixer.iter() {
                            device_chunks
                                .entry(id.clone())
                                .or_insert_with(Vec::new)
                                .push(samples.len());
                        }

                        let sample_details: Vec<String> = device_chunks
                            .iter()
                            .map(|(id, chunks)| {
                                if chunks.len() == 1 {
                                    format!("{}: {} samples", id, chunks[0])
                                } else {
                                    format!("{}: {} chunks {:?}", id, chunks.len(), chunks)
                                }
                            })
                            .collect();
                        info!(
                            "🎛️ {}: Preparing to mix {} total chunks from {} devices: [{}]",
                            "PRE_MIX".magenta(),
                            input_samples_for_mixer.len(),
                            device_chunks.len(),
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
                                    static QUEUE_FULL_LOG: std::sync::atomic::AtomicU64 =
                                        std::sync::atomic::AtomicU64::new(0);
                                    let log_count = QUEUE_FULL_LOG
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    if log_count % 1000 == 0 {
                                        warn!(
                                            "⚠️ {}: Output '{}' RTRB queue full, dropping {} remaining samples (occurrence #{})",
                                            "MIXING_LAYER".on_green().white(),
                                            device_id,
                                            remaining.len(),
                                            log_count
                                        );
                                    }
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

                // Yield whenever no block was produced, which includes waiting for
                // output room. Without this the backpressure check spins hot.
                if !produced_block {
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

    /// Size the mix block to an output's hardware buffer
    ///
    /// The tightest output wins. A block bigger than what an output's hardware
    /// takes per callback would make the mixer produce faster than that output
    /// can drain; smaller just means it does slightly more work per unit audio.
    /// Takes effect on the mixing thread's next cycle.
    pub fn constrain_mix_block_to(&mut self, output_frames: usize) {
        let requested = output_frames * 2; // the mix is stereo
        if requested == 0 {
            return;
        }

        let current = self.mix_block_samples.load(Ordering::Relaxed);
        if requested >= current {
            return;
        }

        self.mix_block_samples.store(requested, Ordering::Relaxed);
        info!(
            "🎛️ {}: Mix block constrained to {} samples ({} frames) by an output",
            "MIXING_LAYER".on_green().white(),
            requested,
            output_frames
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
