// Bus routing commands
//
// A bus is a named mix that inputs send to and outputs take. Every device joins
// the main bus when it registers, so a session nobody has routed behaves as the
// single shared mix it always was.

use tauri::State;

use crate::audio::mixer::pipeline::bus_routing::Bus;
use crate::audio::mixer::stream_management::isolated_audio_manager::BusCommand;
use crate::audio::mixer::stream_management::AudioCommand;
use crate::db::{AudioBusService, AudioMixerConfigurationService};
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

/// Write the mixing layer's routing to the active session
///
/// Snapshotting the engine rather than mirroring each edit means the stored
/// table can only ever be a state the mixer was actually in, including the
/// changes a routing edit makes on its own — removing a bus moves its outputs
/// back to main, and that has to be recorded too.
///
/// A failure here is logged rather than returned: the routing change itself has
/// already taken effect, and reporting it as failed would be worse than losing
/// it on restart.
async fn persist(state: &State<'_, AudioState>) {
    let session =
        match AudioMixerConfigurationService::get_active_session(state.database.sea_orm()).await {
            Ok(Some(session)) => session,
            Ok(None) => {
                tracing::debug!("No active session, so routing has nowhere to be saved");
                return;
            }
            Err(e) => {
                tracing::warn!("Could not read the active session to save routing: {}", e);
                return;
            }
        };

    let buses = match dispatch(state, |response_tx| {
        AudioCommand::Bus(BusCommand::List { response_tx })
    })
    .await
    {
        Ok(buses) => buses,
        Err(e) => {
            tracing::warn!("Could not read routing back to save it: {}", e);
            return;
        }
    };

    if let Err(e) =
        AudioBusService::save_for_configuration(state.database.sea_orm(), &session.id, &buses).await
    {
        tracing::warn!("Could not save routing: {}", e);
    }
}

/// Every bus, with the inputs feeding it and the outputs taking it
#[tauri::command]
pub async fn list_audio_buses(state: State<'_, AudioState>) -> Result<Vec<Bus>, String> {
    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::List { response_tx })
    })
    .await
}

/// Lay the active session's stored routing over the devices already registered
///
/// Call once the session's devices are in place. Devices register onto the main
/// bus, and this is what moves them to where they were left; a device the stored
/// routing says nothing about stays where it is.
#[tauri::command]
pub async fn restore_audio_buses(state: State<'_, AudioState>) -> Result<Vec<Bus>, String> {
    log_command!("restore_audio_buses");

    let Some(session) =
        AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
            .await
            .map_err(|e| e.to_string())?
    else {
        return list_audio_buses(state).await;
    };

    let stored = AudioBusService::load_for_configuration(state.database.sea_orm(), &session.id)
        .await
        .map_err(|e| e.to_string())?;

    if stored.is_empty() {
        return list_audio_buses(state).await;
    }

    dispatch(&state, |response_tx| {
        AudioCommand::Bus(BusCommand::Restore {
            buses: stored,
            response_tx,
        })
    })
    .await?;

    list_audio_buses(state).await
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
    .await?;

    persist(&state).await;
    Ok(())
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
    .await?;

    persist(&state).await;
    Ok(())
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
    .await?;

    persist(&state).await;
    Ok(())
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
    .await?;

    persist(&state).await;
    Ok(())
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
    .await?;

    persist(&state).await;
    Ok(())
}
