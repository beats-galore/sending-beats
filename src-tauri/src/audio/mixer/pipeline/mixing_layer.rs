// Layer 3: Mixing Layer
//
// Single-threaded mixer that:
// 1. Receives processed audio from all Layer 2 input workers
// 2. Mixes/sums all input streams together
// 3. Applies master effects and gain
// 4. Sends mixed audio to all Layer 4 output workers

use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::audio_worker::target_downstream_samples;
use super::block_accumulator::BlockAccumulator;
use super::bus_mixer::{BusMixer, OutputSinks};
use super::pacing::jitter_cushion_samples;
use super::realtime_thread;
use crate::audio::mixer::latency_probe::{LatencyProbe, LatencyStage, StageGauge};
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::VUChannelService;
use colored::*;

mod routing;
use routing::log_bus_result;

/// Block size before any output device has said what its hardware wants
const DEFAULT_MIX_BLOCK_SAMPLES: usize = 1024;

/// Blocks an output may hold before the mixer stops producing
///
/// Production waits for *every* output to be under this at once, so it cannot be
/// a single block: outputs drain on independent hardware clocks and are rarely
/// all empty at the same instant. See the gate in the worker loop.
const OUTPUT_TARGET_BLOCKS: usize = 3;

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
    CreateBus {
        bus_id: String,
        name: String,
    },
    RemoveBus {
        bus_id: String,
    },
    SetBusGain {
        bus_id: String,
        gain: f32,
    },
    SetInputSends {
        device_id: String,
        bus_ids: Vec<String>,
    },
    SetOutputBus {
        device_id: String,
        bus_id: String,
    },
    RestoreRouting {
        buses: Vec<super::bus_routing::Bus>,
    },
    SetOutputSources {
        device_id: String,
        input_ids: Vec<String>,
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

    // Which inputs feed which bus, and which outputs take it. Handed to the
    // mixing thread on start and mutated through commands from then on.
    bus_mixer: BusMixer,

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
    worker_handle: Option<std::thread::JoinHandle<()>>,
    /// Cleared to ask the mixing thread to finish its current cycle and return
    running: Arc<AtomicBool>,

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
            bus_mixer: BusMixer::new(),
            command_tx,
            target_sample_rate: Arc::new(AtomicU32::new(0)),
            master_gain: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            mix_block_samples: Arc::new(AtomicUsize::new(DEFAULT_MIX_BLOCK_SAMPLES)),
            latency_probe,
            worker_handle: None,
            running: Arc::new(AtomicBool::new(true)),
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
        self.bus_mixer
            .registry_mut()
            .attach_input(device_id.clone());

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
        self.bus_mixer.registry_mut().detach_input(&device_id);

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
        self.bus_mixer
            .registry_mut()
            .attach_output(device_id.clone());

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
        self.bus_mixer.registry_mut().detach_output(&device_id);

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
        let mut bus_mixer = BusMixer::from_registry(self.bus_mixer.registry().clone());
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
        self.running.store(true, Ordering::Relaxed);
        let running = self.running.clone();

        // One mix block's worth of audio is the deadline this thread works to
        let block_frames = self.mix_block_samples.load(Ordering::Relaxed) / 2;
        let work_period =
            std::time::Duration::from_secs_f64(block_frames as f64 / current_sample_rate as f64);
        let idle_poll = (work_period / 4).max(std::time::Duration::from_micros(100));

        let worker_handle = realtime_thread::spawn("mixing-layer", work_period, move || {
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

            while running.load(Ordering::Relaxed) {
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
                            bus_mixer.registry_mut().attach_input(device_id.clone());
                            info!(
                                "🎛️ MIXING_LAYER_WORKER: Added input consumer for device '{}'",
                                device_id
                            );
                        }
                        MixingLayerCommand::RemoveInputStream { device_id } => {
                            input_rtrb_consumers.remove(&device_id);
                            input_queue_trackers.remove(&device_id);
                            bus_mixer.registry_mut().detach_input(&device_id);
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
                            bus_mixer.registry_mut().attach_output(device_id.clone());
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
                            bus_mixer.registry_mut().detach_output(&device_id);
                            latency_probe.remove_device(&device_id);
                            info!(
                                "🗑️ MIXING_LAYER_WORKER: Removed output producer for device '{}' (remaining: {})",
                                device_id,
                                output_rtrb_producers.len()
                            );
                        }
                        MixingLayerCommand::CreateBus { bus_id, name } => {
                            log_bus_result(
                                "create bus",
                                &bus_id,
                                bus_mixer.registry_mut().create(bus_id.clone(), name),
                            );
                        }
                        MixingLayerCommand::RemoveBus { bus_id } => {
                            log_bus_result(
                                "remove bus",
                                &bus_id,
                                bus_mixer.registry_mut().remove(&bus_id),
                            );
                        }
                        MixingLayerCommand::SetBusGain { bus_id, gain } => {
                            log_bus_result(
                                "set bus gain",
                                &bus_id,
                                bus_mixer.registry_mut().set_gain(&bus_id, gain),
                            );
                        }
                        MixingLayerCommand::SetInputSends { device_id, bus_ids } => {
                            log_bus_result(
                                "set input sends",
                                &device_id,
                                bus_mixer
                                    .registry_mut()
                                    .set_input_sends(&device_id, &bus_ids),
                            );
                        }
                        MixingLayerCommand::SetOutputSources {
                            device_id,
                            input_ids,
                        } => {
                            let bus_id = bus_mixer
                                .registry_mut()
                                .set_output_sources(&device_id, &input_ids);
                            info!(
                                "🔀 {}: '{}' now takes {} inputs on bus '{}'",
                                "BUS_ROUTING".on_green().white(),
                                device_id,
                                input_ids.len(),
                                bus_id
                            );
                        }
                        MixingLayerCommand::RestoreRouting { buses } => {
                            bus_mixer.registry_mut().restore(&buses);
                            info!(
                                "🔀 {}: restored {} buses",
                                "BUS_ROUTING".on_green().white(),
                                buses.len()
                            );
                        }
                        MixingLayerCommand::SetOutputBus { device_id, bus_id } => {
                            log_bus_result(
                                "set output bus",
                                &device_id,
                                bus_mixer.registry_mut().set_output_bus(&device_id, &bus_id),
                            );
                        }
                    }
                }
                let command_duration = command_start.elapsed();

                // **STEP 1**: Collect samples from RTRB and accumulate per device
                let collection_start = std::time::Instant::now();
                for (device_id, consumer) in input_rtrb_consumers.iter() {
                    let mut consumer_lock = match consumer.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
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
                //
                // The target has to cover more than a single block. Every output must
                // be under it at the same instant, and outputs drain on their own
                // hardware clocks — so a one-block target makes the mix wait for every
                // ring to be empty simultaneously, a window that narrows with each
                // output added. At two outputs it halves the production rate and the
                // render callback pads the missing half with silence, which is what
                // the crunch is. A few blocks in hand lets each output ride its own
                // clock, at the cost of those blocks as standing delay — about a
                // millisecond each at 48k stereo.
                let sync_start = std::time::Instant::now();

                let target_queued = target_downstream_samples(block_samples, mix_rate, 2)
                    .max(block_samples * OUTPUT_TARGET_BLOCKS);
                let mut outputs_ready = !output_rtrb_producers.is_empty();
                for (device_id, producer) in output_rtrb_producers.iter() {
                    let producer_lock = match producer.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
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

                // **STEP 3**: Mix each bus and hand it to the outputs taking it
                //
                // Outputs sharing a bus share its mix, so identically routed
                // destinations cost one sum between them rather than one each.
                let mixing_duration = if let Some(synchronized_samples) = synchronized_samples {
                    let mixing_start = std::time::Instant::now();

                    // Every block is already exactly block_samples long
                    let inputs: Vec<(&str, &[f32])> = synchronized_samples
                        .iter()
                        .map(|(device_id, samples)| (device_id.as_str(), samples.as_slice()))
                        .collect();

                    let master_gain_now = f32::from_bits(master_gain.load(Ordering::Relaxed));
                    let stats = bus_mixer.mix_and_dispatch(
                        &inputs,
                        block_samples,
                        master_gain_now,
                        &OutputSinks {
                            producers: &output_rtrb_producers,
                            trackers: &output_queue_trackers,
                        },
                        master_vu_service.as_ref(),
                    );

                    mix_cycles += 1;
                    let total_mixing_duration = mixing_start.elapsed();

                    if mix_cycles <= 5 || mix_cycles % 1000 == 0 {
                        info!(
                            "🎵 {}: mixed {} inputs into {} buses ({} samples each) across {} outputs (cycle #{}, sync {}μs, total {}μs)",
                            "MIXING_LAYER_TEMPORAL".on_green().white(),
                            inputs.len(),
                            stats.buses_mixed,
                            stats.samples_per_bus,
                            stats.outputs_written,
                            mix_cycles,
                            sync_duration.as_micros(),
                            total_mixing_duration.as_micros()
                        );
                    }

                    if total_mixing_duration.as_micros() > 1000 {
                        warn!(
                            "⏱️ {}: Slow mixing cycle: {}μs across {} buses",
                            "MIXING_LAYER_SLOW".on_green().white(),
                            total_mixing_duration.as_micros(),
                            stats.buses_mixed
                        );
                    }

                    total_mixing_duration
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
                    std::thread::sleep(idle_poll);
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
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.worker_handle.take() {
            match handle.join() {
                Ok(()) => info!(
                    "✅ {}: Shut down gracefully",
                    "MIXING_LAYER".on_green().white()
                ),
                Err(_) => warn!(
                    "⚠️ {}: Panicked before shutting down",
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
