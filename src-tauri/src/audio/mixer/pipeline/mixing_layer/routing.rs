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
    /// Apply a routing change, on the mixing thread if it is running
    ///
    /// Before the thread starts the registry is still local, so the change is
    /// made directly and reports its own error. Once running the registry has
    /// moved onto the thread, and the outcome is only visible in its log.
    fn route(
        &mut self,
        command: MixingLayerCommand,
        apply: impl FnOnce(&mut BusMixer) -> Result<(), BusError>,
    ) -> Result<(), BusError> {
        if self.worker_handle.is_some() {
            if self.command_tx.send(command).is_err() {
                warn!(
                    "⚠️ {}: Failed to send routing command",
                    "MIXING_LAYER".on_green().white()
                );
            }
            return Ok(());
        }

        apply(&mut self.bus_mixer)
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

    /// Every bus and its members
    ///
    /// Only meaningful before the mixing thread starts, which takes ownership of
    /// the registry. Reporting live routing back to the frontend needs the
    /// registry to be shared rather than moved.
    pub fn buses(&self) -> Vec<Bus> {
        self.bus_mixer.registry().buses().cloned().collect()
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
