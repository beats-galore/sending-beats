// Routing commands on the mixing layer
//
// A child module so it can reach MixingLayer's private fields while keeping the
// mixing thread itself in one readable file.

use tracing::{info, warn};

use super::super::bus_mixer::BusMixer;
use super::super::bus_routing::{Bus, BusError};
use super::{MixingLayer, MixingLayerCommand};
use colored::*;

impl MixingLayer {
    /// Apply a routing change here, then forward it to the mixing thread
    ///
    /// The layer's own registry is applied first and is what a caller's error
    /// comes from, so an invalid change is rejected before any command is sent
    /// and the two copies cannot disagree about whether it happened.
    fn route(
        &mut self,
        command: MixingLayerCommand,
        apply: impl FnOnce(&mut BusMixer) -> Result<(), BusError>,
    ) -> Result<(), BusError> {
        apply(&mut self.bus_mixer)?;

        if self.worker_handle.is_some() && self.command_tx.send(command).is_err() {
            warn!(
                "⚠️ {}: Failed to send routing command",
                "MIXING_LAYER".on_green().white()
            );
        }

        Ok(())
    }

    pub fn create_bus(&mut self, bus_id: String, name: String) -> Result<(), BusError> {
        let command = MixingLayerCommand::CreateBus {
            bus_id: bus_id.clone(),
            name: name.clone(),
        };
        self.route(command, |mixer| mixer.registry_mut().create(bus_id, name))
    }

    pub fn remove_bus(&mut self, bus_id: String) -> Result<(), BusError> {
        let command = MixingLayerCommand::RemoveBus {
            bus_id: bus_id.clone(),
        };
        self.route(command, |mixer| mixer.registry_mut().remove(&bus_id))
    }

    pub fn set_bus_gain(&mut self, bus_id: String, gain: f32) -> Result<(), BusError> {
        let command = MixingLayerCommand::SetBusGain {
            bus_id: bus_id.clone(),
            gain,
        };
        self.route(command, |mixer| {
            mixer.registry_mut().set_gain(&bus_id, gain)
        })
    }

    pub fn set_input_sends(
        &mut self,
        device_id: String,
        bus_ids: Vec<String>,
    ) -> Result<(), BusError> {
        let command = MixingLayerCommand::SetInputSends {
            device_id: device_id.clone(),
            bus_ids: bus_ids.clone(),
        };
        self.route(command, |mixer| {
            mixer.registry_mut().set_input_sends(&device_id, &bus_ids)
        })
    }

    pub fn set_output_bus(&mut self, device_id: String, bus_id: String) -> Result<(), BusError> {
        let command = MixingLayerCommand::SetOutputBus {
            device_id: device_id.clone(),
            bus_id: bus_id.clone(),
        };
        self.route(command, |mixer| {
            mixer.registry_mut().set_output_bus(&device_id, &bus_id)
        })
    }

    /// Point an output at exactly the inputs it should receive
    ///
    /// What the patchbay's tiles write, from either side of the connection.
    pub fn set_output_sources(
        &mut self,
        device_id: String,
        input_ids: Vec<String>,
    ) -> Result<(), BusError> {
        let command = MixingLayerCommand::SetOutputSources {
            device_id: device_id.clone(),
            input_ids: input_ids.clone(),
        };
        self.route(command, |mixer| {
            mixer
                .registry_mut()
                .set_output_sources(&device_id, &input_ids);
            Ok(())
        })
    }

    /// Lay stored routing over the devices currently registered
    pub fn restore_routing(&mut self, buses: Vec<Bus>) -> Result<(), BusError> {
        let command = MixingLayerCommand::RestoreRouting {
            buses: buses.clone(),
        };
        self.route(command, |mixer| {
            mixer.registry_mut().restore(&buses);
            Ok(())
        })
    }

    /// Every bus and its members
    ///
    /// Answered from the layer's own registry, which every change passes
    /// through before reaching the mixing thread, so it describes what the
    /// mixer is doing without the audio path taking a lock to be read.
    pub fn buses(&self) -> Vec<Bus> {
        self.bus_mixer.registry().buses().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::MixingLayer;
    use crate::audio::mixer::latency_probe::LatencyProbe;
    use crate::audio::mixer::pipeline::bus_routing::{BusError, MAIN_BUS_ID};
    use crate::audio::mixer::queue_manager::AtomicQueueTracker;
    use std::sync::{Arc, Mutex};

    fn layer() -> MixingLayer {
        MixingLayer::new(LatencyProbe::new())
    }

    fn add_input(layer: &mut MixingLayer, device_id: &str) {
        let (_producer, consumer) = rtrb::RingBuffer::<f32>::new(16);
        layer.add_input_consumer(
            device_id.to_string(),
            Arc::new(Mutex::new(consumer)),
            AtomicQueueTracker::new(format!("{}_in", device_id), 16),
        );
    }

    fn add_output(layer: &mut MixingLayer, device_id: &str) {
        let (producer, _consumer) = rtrb::RingBuffer::<f32>::new(16);
        layer.add_output_producer(
            device_id.to_string(),
            Arc::new(Mutex::new(producer)),
            AtomicQueueTracker::new(format!("{}_out", device_id), 16),
        );
    }

    fn bus<'a>(
        layer: &'a MixingLayer,
        bus_id: &str,
    ) -> crate::audio::mixer::pipeline::bus_routing::Bus {
        layer
            .buses()
            .into_iter()
            .find(|b| b.id == bus_id)
            .expect("bus should exist")
    }

    /// The bus a destination is on, which is the only one it can be on
    fn bus_of<'a>(
        layer: &'a MixingLayer,
        output_id: &str,
    ) -> crate::audio::mixer::pipeline::bus_routing::Bus {
        layer
            .buses()
            .into_iter()
            .find(|b| b.outputs.contains(output_id))
            .expect("output should be on a bus")
    }

    #[test]
    fn registered_devices_appear_on_the_main_bus() {
        let mut layer = layer();
        add_input(&mut layer, "mic");
        add_output(&mut layer, "speakers");

        let main = bus(&layer, MAIN_BUS_ID);
        assert!(main.inputs.contains("mic"));
        assert!(main.outputs.contains("speakers"));
    }

    #[test]
    fn routing_reads_back_what_was_set() {
        let mut layer = layer();
        add_input(&mut layer, "mic");
        add_input(&mut layer, "deck");
        add_output(&mut layer, "speakers");
        add_output(&mut layer, "headphones");

        layer
            .set_output_sources("headphones".to_string(), vec!["deck".to_string()])
            .unwrap();

        // Routing one destination away splits the mix rather than editing the
        // one the other destination is still taking.
        let cue = bus_of(&layer, "headphones");
        assert!(cue.inputs.contains("deck"));
        assert!(!cue.inputs.contains("mic"));
        assert_ne!(cue.id, MAIN_BUS_ID);

        let main = bus(&layer, MAIN_BUS_ID);
        assert!(main.outputs.contains("speakers"));
        assert!(main.inputs.contains("mic"));
    }

    #[test]
    fn an_invalid_change_is_rejected_rather_than_reported_as_applied() {
        let mut layer = layer();
        add_input(&mut layer, "mic");
        add_output(&mut layer, "speakers");

        let result = layer.set_input_sends("mic".to_string(), vec!["nope".to_string()]);

        assert_eq!(result, Err(BusError::UnknownBus("nope".to_string())));
        assert!(bus(&layer, MAIN_BUS_ID).inputs.contains("mic"));
    }

    #[tokio::test]
    async fn routing_survives_the_mixing_thread_starting() {
        let mut layer = layer();
        add_input(&mut layer, "mic");
        add_input(&mut layer, "deck");
        add_output(&mut layer, "speakers");
        add_output(&mut layer, "headphones");
        layer
            .set_output_sources("headphones".to_string(), vec!["deck".to_string()])
            .unwrap();
        layer.update_target_sample_rate(48_000);

        layer.start(crate::audio::new_shared_vu_channel()).unwrap();

        // The thread works from a copy, so the layer can still be asked what the
        // routing is rather than reporting an empty table
        let main = bus(&layer, MAIN_BUS_ID);
        assert!(main.inputs.contains("mic"));
        assert!(main.outputs.contains("speakers"));
        assert_eq!(layer.buses().len(), 2);

        layer.stop().await.unwrap();
    }

    #[tokio::test]
    async fn a_device_added_while_running_is_still_visible() {
        let mut layer = layer();
        add_output(&mut layer, "speakers");
        layer.update_target_sample_rate(48_000);
        layer.start(crate::audio::new_shared_vu_channel()).unwrap();

        add_input(&mut layer, "late-mic");

        // An unrouted destination takes whatever is attached, so a source added
        // after the fact is audible without being routed by hand.
        assert!(bus(&layer, MAIN_BUS_ID).inputs.contains("late-mic"));

        layer.stop().await.unwrap();
    }
}

/// Report a routing change applied on the mixing thread, where no caller is left to return to
pub(super) fn log_bus_result(action: &str, subject: &str, result: Result<(), BusError>) {
    match result {
        Ok(()) => info!(
            "🔀 {}: {} '{}'",
            "BUS_ROUTING".on_green().white(),
            action,
            subject
        ),
        Err(e) => warn!(
            "⚠️ {}: could not {} '{}': {}",
            "BUS_ROUTING".on_green().white(),
            action,
            subject,
            e
        ),
    }
}
