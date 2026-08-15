// Summing a block of input audio into each bus and handing it to that bus's outputs
//
// One mix per bus rather than one per output: outputs that take the same bus
// share its buffer, so a configuration is summed once however many destinations
// it reaches.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::warn;

use super::bus_routing::{BusRegistry, MAIN_BUS_ID};
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::stream_management::virtual_mixer::VirtualMixer;
use crate::audio::VUChannelService;
use colored::*;

/// The output-side halves of the queues a bus writes into
pub struct OutputSinks<'a> {
    pub producers: &'a HashMap<String, Arc<Mutex<rtrb::Producer<f32>>>>,
    pub trackers: &'a HashMap<String, AtomicQueueTracker>,
}

/// What one dispatch cycle did, for logging
#[derive(Debug, Default, Clone, Copy)]
pub struct DispatchStats {
    pub buses_mixed: usize,
    pub outputs_written: usize,
    pub samples_per_bus: usize,
}

pub struct BusMixer {
    registry: BusRegistry,
    /// Reused for buses with nothing routed to them, so an idle bus does not
    /// allocate a fresh block of silence every cycle
    silence: Vec<f32>,
}

impl BusMixer {
    pub fn new() -> Self {
        Self::from_registry(BusRegistry::new())
    }

    /// A mixer working from an existing routing table
    ///
    /// The mixing thread is given a copy of the layer's registry rather than
    /// taking it, so the layer keeps one it can still answer queries from.
    pub fn from_registry(registry: BusRegistry) -> Self {
        Self {
            registry,
            silence: Vec::new(),
        }
    }

    pub fn registry(&self) -> &BusRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut BusRegistry {
        &mut self.registry
    }

    /// Mix every bus that something takes, and write it to the outputs taking it
    ///
    /// `inputs` holds one block per input device, already stereo and at the mix
    /// rate. `master_gain` is folded into each bus's own gain so the samples are
    /// only walked once.
    pub fn mix_and_dispatch(
        &mut self,
        inputs: &[(&str, &[f32])],
        block_samples: usize,
        master_gain: f32,
        sinks: &OutputSinks<'_>,
        vu: Option<&VUChannelService>,
    ) -> DispatchStats {
        // Split the borrow so the silence buffer stays writable while the
        // registry is being walked
        let Self { registry, silence } = self;

        let mut stats = DispatchStats::default();
        let mut routed: Vec<(&str, &[f32])> = Vec::with_capacity(inputs.len());

        for bus in registry.buses() {
            // Nothing takes this bus, so mixing it would be work for no one.
            // Its meter is fed by the bus's own outputs once per-bus metering
            // exists; until then only the main bus is metered.
            if bus.outputs.is_empty() {
                continue;
            }

            routed.clear();
            routed.extend(
                inputs
                    .iter()
                    .filter(|(device_id, _)| bus.inputs.contains(*device_id))
                    .copied(),
            );

            // A bus with nothing routed to it still owes its outputs a block.
            // Writing nothing starves them and the output workers underrun.
            let mixed;
            let block: &[f32] = if routed.is_empty() {
                if silence.len() != block_samples {
                    silence.resize(block_samples, 0.0);
                }
                silence.as_slice() // any gain on silence is still silence
            } else {
                mixed = apply_gain(
                    VirtualMixer::mix_input_samples_ref(&routed),
                    bus.gain * master_gain,
                );
                &mixed
            };

            // Metered after the bus's own gain, so the reading is what the
            // outputs taking it are being handed. The main bus additionally
            // feeds the master meter, which predates buses and is what the
            // frontend reads today.
            if let Some(vu_service) = vu {
                vu_service.queue_bus_audio(&bus.id, block);
                if bus.id == MAIN_BUS_ID {
                    vu_service.queue_master_audio(block);
                }
            }

            for output_id in bus.outputs.iter() {
                let Some(producer) = sinks.producers.get(output_id) else {
                    continue;
                };

                let written = write_block(output_id, producer, block);
                if let Some(tracker) = sinks.trackers.get(output_id) {
                    tracker.record_samples_written(written);
                }
                stats.outputs_written += 1;
            }

            stats.buses_mixed += 1;
            stats.samples_per_bus = block.len();
        }

        stats
    }
}

impl Default for BusMixer {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_gain(mut block: Vec<f32>, gain: f32) -> Vec<f32> {
    if gain != 1.0 {
        for sample in block.iter_mut() {
            *sample *= gain;
        }
    }
    block
}

/// Push a block into an output's ring, returning how much of it landed
fn write_block(
    output_id: &str,
    producer: &Arc<Mutex<rtrb::Producer<f32>>>,
    block: &[f32],
) -> usize {
    let mut producer_lock = match producer.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let mut samples_written = 0;
    let mut remaining = block;

    while !remaining.is_empty() {
        let chunk_size = remaining.len().min(producer_lock.slots());
        if chunk_size == 0 {
            static QUEUE_FULL_LOG: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let log_count = QUEUE_FULL_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if log_count % 1000 == 0 {
                warn!(
                    "⚠️ {}: Output '{}' RTRB queue full, dropping {} remaining samples (occurrence #{})",
                    "BUS_MIXER".on_green().white(),
                    output_id,
                    remaining.len(),
                    log_count
                );
            }
            break;
        }

        for &sample in &remaining[..chunk_size] {
            if producer_lock.push(sample).is_err() {
                break;
            }
            samples_written += 1;
        }
        remaining = &remaining[chunk_size..];
    }

    samples_written
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: usize = 4;

    struct Harness {
        producers: HashMap<String, Arc<Mutex<rtrb::Producer<f32>>>>,
        trackers: HashMap<String, AtomicQueueTracker>,
        consumers: HashMap<String, rtrb::Consumer<f32>>,
    }

    impl Harness {
        fn new(output_ids: &[&str]) -> Self {
            let mut producers = HashMap::new();
            let mut trackers = HashMap::new();
            let mut consumers = HashMap::new();

            for id in output_ids {
                let (producer, consumer) = rtrb::RingBuffer::<f32>::new(BLOCK * 8);
                producers.insert((*id).to_string(), Arc::new(Mutex::new(producer)));
                trackers.insert(
                    (*id).to_string(),
                    AtomicQueueTracker::new(format!("{}_test", id), BLOCK * 8),
                );
                consumers.insert((*id).to_string(), consumer);
            }

            Self {
                producers,
                trackers,
                consumers,
            }
        }

        fn sinks(&self) -> OutputSinks<'_> {
            OutputSinks {
                producers: &self.producers,
                trackers: &self.trackers,
            }
        }

        fn drain(&mut self, output_id: &str) -> Vec<f32> {
            let consumer = self.consumers.get_mut(output_id).unwrap();
            let mut out = Vec::new();
            while let Ok(sample) = consumer.pop() {
                out.push(sample);
            }
            out
        }
    }

    #[test]
    fn every_output_gets_the_same_mix_by_default() {
        let mut mixer = BusMixer::new();
        mixer.registry_mut().attach_input("mic".to_string());
        mixer.registry_mut().attach_output("speakers".to_string());
        mixer.registry_mut().attach_output("stream".to_string());

        let mut harness = Harness::new(&["speakers", "stream"]);
        let mic = [0.25f32; BLOCK];

        let stats = mixer.mix_and_dispatch(&[("mic", &mic)], BLOCK, 1.0, &harness.sinks(), None);

        assert_eq!(stats.buses_mixed, 1, "one bus means one mix, not two");
        assert_eq!(harness.drain("speakers"), vec![0.25; BLOCK]);
        assert_eq!(harness.drain("stream"), vec![0.25; BLOCK]);
    }

    #[test]
    fn an_input_reaches_only_the_outputs_on_its_bus() {
        let mut mixer = BusMixer::new();
        let registry = mixer.registry_mut();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("headphones".to_string());
        registry
            .set_input_sends("deck", &["cue".to_string()])
            .unwrap();
        registry.set_output_bus("headphones", "cue").unwrap();

        let mut harness = Harness::new(&["speakers", "headphones"]);
        let mic = [0.1f32; BLOCK];
        let deck = [0.7f32; BLOCK];

        let stats = mixer.mix_and_dispatch(
            &[("mic", &mic), ("deck", &deck)],
            BLOCK,
            1.0,
            &harness.sinks(),
            None,
        );

        assert_eq!(stats.buses_mixed, 2);
        assert_eq!(harness.drain("speakers"), vec![0.1; BLOCK], "deck excluded");
        assert_eq!(
            harness.drain("headphones"),
            vec![0.7; BLOCK],
            "mic excluded"
        );
    }

    #[test]
    fn an_input_sending_to_both_buses_reaches_both() {
        let mut mixer = BusMixer::new();
        let registry = mixer.registry_mut();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_input("mic".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("headphones".to_string());
        registry
            .set_input_sends("mic", &[MAIN_BUS_ID.to_string(), "cue".to_string()])
            .unwrap();
        registry.set_output_bus("headphones", "cue").unwrap();

        let mut harness = Harness::new(&["speakers", "headphones"]);
        let mic = [0.5f32; BLOCK];

        mixer.mix_and_dispatch(&[("mic", &mic)], BLOCK, 1.0, &harness.sinks(), None);

        assert_eq!(harness.drain("speakers"), vec![0.5; BLOCK]);
        assert_eq!(harness.drain("headphones"), vec![0.5; BLOCK]);
    }

    #[test]
    fn a_bus_with_no_inputs_still_feeds_its_outputs_silence() {
        let mut mixer = BusMixer::new();
        let registry = mixer.registry_mut();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_output("headphones".to_string());
        registry.set_output_bus("headphones", "cue").unwrap();

        let mut harness = Harness::new(&["headphones"]);

        mixer.mix_and_dispatch(&[], BLOCK, 1.0, &harness.sinks(), None);

        // Writing nothing here would starve the output worker into an underrun
        assert_eq!(harness.drain("headphones"), vec![0.0; BLOCK]);
    }

    #[test]
    fn a_bus_nothing_takes_is_not_mixed() {
        let mut mixer = BusMixer::new();
        let registry = mixer.registry_mut();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_input("mic".to_string());
        registry
            .set_input_sends("mic", &["cue".to_string()])
            .unwrap();

        let harness = Harness::new(&[]);
        let mic = [0.5f32; BLOCK];

        let stats = mixer.mix_and_dispatch(&[("mic", &mic)], BLOCK, 1.0, &harness.sinks(), None);

        assert_eq!(stats.buses_mixed, 0);
    }

    #[test]
    fn inputs_on_a_bus_are_summed() {
        let mut mixer = BusMixer::new();
        mixer.registry_mut().attach_input("a".to_string());
        mixer.registry_mut().attach_input("b".to_string());
        mixer.registry_mut().attach_output("speakers".to_string());

        let mut harness = Harness::new(&["speakers"]);
        let a = [0.2f32; BLOCK];
        let b = [0.3f32; BLOCK];

        mixer.mix_and_dispatch(&[("a", &a), ("b", &b)], BLOCK, 1.0, &harness.sinks(), None);

        let written = harness.drain("speakers");
        assert_eq!(written.len(), BLOCK);
        for sample in written {
            assert!((sample - 0.5).abs() < 1e-6, "got {}", sample);
        }
    }

    #[test]
    fn bus_gain_and_master_gain_both_apply() {
        let mut mixer = BusMixer::new();
        mixer.registry_mut().attach_input("mic".to_string());
        mixer.registry_mut().attach_output("speakers".to_string());
        mixer.registry_mut().set_gain(MAIN_BUS_ID, 0.5).unwrap();

        let mut harness = Harness::new(&["speakers"]);
        let mic = [0.8f32; BLOCK];

        mixer.mix_and_dispatch(&[("mic", &mic)], BLOCK, 0.5, &harness.sinks(), None);

        for sample in harness.drain("speakers") {
            assert!((sample - 0.2).abs() < 1e-6, "got {}", sample);
        }
    }

    #[test]
    fn an_input_on_no_bus_reaches_nothing() {
        let mut mixer = BusMixer::new();
        let registry = mixer.registry_mut();
        registry.attach_input("mic".to_string());
        registry.attach_output("speakers".to_string());
        registry.set_input_sends("mic", &[]).unwrap();

        let mut harness = Harness::new(&["speakers"]);
        let mic = [0.9f32; BLOCK];

        mixer.mix_and_dispatch(&[("mic", &mic)], BLOCK, 1.0, &harness.sinks(), None);

        assert_eq!(harness.drain("speakers"), vec![0.0; BLOCK]);
    }
}
