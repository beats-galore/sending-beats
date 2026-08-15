// Bus routing commands
//
// A bus is a named mix that inputs send to and outputs take. Every device joins
// the main bus when it registers, so a session nobody has routed behaves as the
// single shared mix it always was.

use tauri::State;

use crate::audio::mixer::pipeline::bus_routing::Bus;
use crate::audio::mixer::stream_management::isolated_audio_manager::BusCommand;
use crate::audio::mixer::stream_management::AudioCommand;
use crate::log_command;
use crate::AudioState;
use colored::*;

/// Send a command to the audio coordinator and wait for what it made of it
///
/// Routing is owned by the mixing layer, which only takes instructions on its
/// command channel, so every one of these is a round trip rather than a call.
async fn dispatch<T>(
    state: &State<'_, AudioState>,
    build: impl FnOnce(tokio::sync::oneshot::Sender<anyhow::Result<T>>) -> AudioCommand,
) -> Result<T, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .audio_command_tx
        .send(build(tx))
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Every bus, with the inputs feeding it and the outputs taking it
#[tauri::command]
pub async fn list_audio_buses(state: State<'_, AudioState>) -> Result<Vec<Bus>, String> {
    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::List { response_tx })
    })
    .await
}

#[tauri::command]
pub async fn create_audio_bus(
    bus_id: String,
    name: String,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!("create_audio_bus", "{} ({})", bus_id, name);

    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::Create {
            bus_id,
            name,
            response_tx,
        })
    })
    .await
}

/// Remove a bus, moving whatever took it back to the main bus
#[tauri::command]
pub async fn remove_audio_bus(bus_id: String, state: State<'_, AudioState>) -> Result<(), String> {
    log_command!("remove_audio_bus", "{}", bus_id);

    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::Remove {
            bus_id,
            response_tx,
        })
    })
    .await
}

#[tauri::command]
pub async fn set_audio_bus_gain(
    bus_id: String,
    gain: f32,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!("set_audio_bus_gain", "{} to {:.2}", bus_id, gain);

    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::SetGain {
            bus_id,
            gain,
            response_tx,
        })
    })
    .await
}

/// Replace the set of buses an input sends to
///
/// An empty list is meaningful: it leaves the input reaching nothing, which is
/// how a source is taken off the air without being removed.
#[tauri::command]
pub async fn set_input_bus_sends(
    device_id: String,
    bus_ids: Vec<String>,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!(
        "set_input_bus_sends",
        "{} to [{}]",
        device_id,
        bus_ids.join(", ")
    );

    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::SetInputSends {
            device_id,
            bus_ids,
            response_tx,
        })
    })
    .await
}

/// Move an output onto a bus, taking it off the one it was on
#[tauri::command]
pub async fn set_output_audio_bus(
    device_id: String,
    bus_id: String,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!("set_output_audio_bus", "{} takes {}", device_id, bus_id);

    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::SetOutputBus {
            device_id,
            bus_id,
            response_tx,
        })
    })
    .await
}
