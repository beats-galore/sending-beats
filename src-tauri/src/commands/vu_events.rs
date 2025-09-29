use colored::*;
use tauri::{ipc::Channel, AppHandle, State};
use tokio::sync::oneshot;

use crate::{AudioState, audio::VUChannelData};

/// Initialize VU level events by passing the AppHandle to the audio system
#[tauri::command]
pub async fn initialize_vu_events(
    app: AppHandle,
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!("{}: initialize_vu_events command called", "VU_EVENTS_INIT".cyan());

    // Send SetAppHandle command to the audio system
    let (response_tx, response_rx) = oneshot::channel();

    let command = crate::audio::mixer::stream_management::AudioCommand::SetAppHandle {
        app_handle: app,
        response_tx,
    };

    tracing::info!("{}: Sending SetAppHandle command to audio system...", "VU_EVENTS_INIT".cyan());

    // Send command to the isolated audio manager
    if let Err(_) = audio_state.audio_command_tx.send(command).await {
        tracing::error!("{}: Failed to send AppHandle command to audio system", "VU_EVENTS_ERROR".red());
        return Err("Failed to send AppHandle to audio system".to_string());
    }

    tracing::info!("{}: AppHandle command sent, waiting for response...", "VU_EVENTS_INIT".cyan());

    // Wait for confirmation
    match response_rx.await {
        Ok(Ok(())) => {
            tracing::info!("{}: VU events initialized successfully (confirmed by audio system)", "VU_EVENTS_SUCCESS".green());
            Ok(())
        }
        Ok(Err(e)) => {
            tracing::error!("{}: Audio system reported error: {}", "VU_EVENTS_ERROR".red(), e);
            Err(format!("Failed to initialize VU events: {}", e))
        }
        Err(_) => {
            tracing::error!("{}: Audio system response timeout", "VU_EVENTS_ERROR".red());
            Err("Audio system response timeout".to_string())
        }
    }
}

/// Initialize VU level streaming using high-performance Tauri channels
/// This replaces the slow event system with channels designed for real-time data
#[tauri::command]
pub async fn initialize_vu_channels(
    channel: Channel<VUChannelData>,
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!("{}: initialize_vu_channels command called", "VU_CHANNELS_INIT".bright_green());

    // Send SetVUChannel command to the audio system
    let (response_tx, response_rx) = oneshot::channel();

    let command = crate::audio::mixer::stream_management::AudioCommand::SetVUChannel {
        channel,
        response_tx,
    };

    tracing::info!("{}: Sending SetVUChannel command to audio system...", "VU_CHANNELS_INIT".bright_green());

    // Send command to the isolated audio manager
    if let Err(_) = audio_state.audio_command_tx.send(command).await {
        tracing::error!("{}: Failed to send VU channel command to audio system", "VU_CHANNELS_ERROR".red());
        return Err("Failed to send VU channel to audio system".to_string());
    }

    tracing::info!("{}: VU channel command sent, waiting for response...", "VU_CHANNELS_INIT".bright_green());

    // Wait for confirmation
    match response_rx.await {
        Ok(Ok(())) => {
            tracing::info!("{}: VU channels initialized successfully", "VU_CHANNELS_SUCCESS".bright_green());
            Ok(())
        }
        Ok(Err(e)) => {
            tracing::error!("{}: Audio system reported error: {}", "VU_CHANNELS_ERROR".red(), e);
            Err(format!("Failed to initialize VU channels: {}", e))
        }
        Err(_) => {
            tracing::error!("{}: Audio system response timeout", "VU_CHANNELS_ERROR".red());
            Err("Audio system response timeout".to_string())
        }
    }
}

