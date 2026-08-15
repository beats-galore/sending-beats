// Bus routing instructions for the audio coordinator
//
// Grouped into one AudioCommand variant rather than six so the coordinator's
// command match stays readable, and handled here where it can still reach the
// manager's pipeline.

use anyhow::Result;
use tokio::sync::oneshot;

use super::IsolatedAudioManager;
use crate::audio::mixer::pipeline::bus_routing::Bus;

pub enum BusCommand {
    Create {
        bus_id: String,
        name: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
    Remove {
        bus_id: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
    SetGain {
        bus_id: String,
        gain: f32,
        response_tx: oneshot::Sender<Result<()>>,
    },
    SetInputSends {
        device_id: String,
        bus_ids: Vec<String>,
        response_tx: oneshot::Sender<Result<()>>,
    },
    SetOutputBus {
        device_id: String,
        bus_id: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
    List {
        response_tx: oneshot::Sender<Result<Vec<Bus>>>,
    },
    Restore {
        buses: Vec<Bus>,
        response_tx: oneshot::Sender<Result<()>>,
    },
}

impl IsolatedAudioManager {
    pub(super) fn handle_bus_command(&mut self, command: BusCommand) {
        match command {
            BusCommand::Create {
                bus_id,
                name,
                response_tx,
            } => {
                let _ = response_tx.send(self.audio_pipeline.create_bus(bus_id, name));
            }
            BusCommand::Remove {
                bus_id,
                response_tx,
            } => {
                let _ = response_tx.send(self.audio_pipeline.remove_bus(bus_id));
            }
            BusCommand::SetGain {
                bus_id,
                gain,
                response_tx,
            } => {
                let _ = response_tx.send(self.audio_pipeline.set_bus_gain(bus_id, gain));
            }
            BusCommand::SetInputSends {
                device_id,
                bus_ids,
                response_tx,
            } => {
                let _ = response_tx.send(self.audio_pipeline.set_input_sends(device_id, bus_ids));
            }
            BusCommand::SetOutputBus {
                device_id,
                bus_id,
                response_tx,
            } => {
                let _ = response_tx.send(self.audio_pipeline.set_output_bus(device_id, bus_id));
            }
            BusCommand::Restore { buses, response_tx } => {
                let _ = response_tx.send(self.audio_pipeline.restore_routing(buses));
            }
            BusCommand::List { response_tx } => {
                let _ = response_tx.send(Ok(self.audio_pipeline.list_buses()));
            }
        }
    }
}
