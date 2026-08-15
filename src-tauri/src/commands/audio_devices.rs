use crate::{log_command, AudioDeviceInfo, AudioState};
use colored::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use tauri::State;

/// Outcome of switching the master output device
///
/// The switch and the system audio diversion fail independently: the output can
/// be live while diversion is refused, which leaves the user hearing every
/// source twice, so the diversion result is reported separately rather than
/// folded into the command's error.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDeviceSwitchResult {
    pub system_audio_diverted: bool,
    /// The driver was set up but coreaudiod restarted underneath this process,
    /// so diversion can only finish once the app is relaunched
    pub restart_required: bool,
    pub diversion_error: Option<String>,
}

/// Tear down an output device's stream and forget its saved configuration.
///
/// Used when a destination is re-pointed at a different device: the old one has
/// to leave the pipeline before the new one joins, or the mixer keeps waiting on
/// an output nobody drains.
async fn remove_output_stream_internal(
    audio_state: &AudioState,
    device_id: &str,
) -> Result<(), String> {
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let command = crate::audio::mixer::stream_management::AudioCommand::RemoveOutputStream {
        device_id: device_id.to_string(),
        response_tx,
    };

    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        let error_msg = format!("Audio system not available - failed to send command: {}", e);
        tracing::error!("{}", error_msg);
        return Err(error_msg);
    }

    match response_rx.await {
        Ok(Ok(())) => {
            if let Err(e) =
                crate::commands::configurations::remove_device_configuration(audio_state, device_id)
                    .await
            {
                tracing::warn!(
                    "Removed output stream '{}' but failed to clear its configuration: {}",
                    device_id,
                    e
                );
            }
            Ok(())
        }
        Ok(Err(e)) => {
            let error_msg = format!("Failed to remove output device {}: {}", device_id, e);
            tracing::error!("{}", error_msg);
            Err(error_msg)
        }
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

/// Route system output to the virtual driver so the mix is not played twice
#[cfg(target_os = "macos")]
async fn divert_system_audio(audio_state: &AudioState) -> OutputDeviceSwitchResult {
    use crate::audio::devices::DiversionOutcome;

    let mut router = audio_state.system_audio_router.lock().await;

    match router.divert_system_audio_to_virtual_device().await {
        Ok(DiversionOutcome::Diverted) => {
            tracing::info!(
                "{} System audio diverted to prevent double playback",
                "OUTPUT_DIVERTED".bright_green()
            );
            OutputDeviceSwitchResult {
                system_audio_diverted: true,
                restart_required: false,
                diversion_error: None,
            }
        }
        Ok(DiversionOutcome::RestartRequired) => {
            tracing::info!(
                "{} Virtual driver set up, relaunch required to finish diversion",
                "OUTPUT_DIVERT_RESTART".bright_yellow()
            );
            OutputDeviceSwitchResult {
                system_audio_diverted: false,
                restart_required: true,
                diversion_error: None,
            }
        }
        Err(e) => {
            tracing::warn!(
                "{} Failed to divert system audio: {}",
                "OUTPUT_DIVERT_WARN".bright_yellow(),
                e
            );
            OutputDeviceSwitchResult {
                system_audio_diverted: false,
                restart_required: false,
                diversion_error: Some(e.to_string()),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn divert_system_audio(_audio_state: &AudioState) -> OutputDeviceSwitchResult {
    OutputDeviceSwitchResult {
        system_audio_diverted: false,
        restart_required: false,
        diversion_error: None,
    }
}

#[tauri::command]
pub async fn enumerate_audio_devices(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<AudioDeviceInfo>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    device_manager
        .enumerate_devices()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_audio_devices(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<AudioDeviceInfo>, String> {
    log_command!("refresh_audio_devices");
    let device_manager = audio_state.device_manager.lock().await;
    // Force a fresh device enumeration
    device_manager
        .enumerate_devices()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_audio_device(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<Option<AudioDeviceInfo>, String> {
    log_command!("get_audio_device", "device: {}", device_id);
    let device_manager = audio_state.device_manager.lock().await;
    Ok(device_manager.get_device(&device_id).await)
}

// Device health monitoring commands
//
// These delegate to the DeviceHealthMonitor owned by AudioDeviceManager, which
// tracks connection state and error counts per device.

/// Health record for a single device, or None if it has never been seen
#[tauri::command]
pub async fn get_device_health(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<Option<crate::audio::devices::DeviceHealth>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    Ok(device_manager.get_device_health(&device_id).await)
}

/// Health records for every device the monitor has seen, keyed by device ID
#[tauri::command]
pub async fn get_all_device_health(
    audio_state: State<'_, AudioState>,
) -> Result<std::collections::HashMap<String, crate::audio::devices::DeviceHealth>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    Ok(device_manager.get_all_device_health().await)
}

#[tauri::command]
pub async fn report_device_error(
    audio_state: State<'_, AudioState>,
    device_id: String,
    error: String,
) -> Result<(), String> {
    log_command!(
        "report_device_error",
        "device: {}, error: {}",
        device_id,
        error
    );

    let device_manager = audio_state.device_manager.lock().await;
    device_manager.report_device_error(&device_id, error).await;

    Ok(())
}

// Device switching commands
#[tauri::command]
pub async fn safe_switch_input_device(
    audio_state: State<'_, AudioState>,
    old_device_id: Option<String>,
    new_device_id: String,
    is_virtual: Option<bool>,
) -> Result<Option<crate::entities::configured_audio_device::Model>, String> {
    log_command!(
        "safe_switch_input_device",
        "old: {:?}, new: {}, virtual: {:?}",
        old_device_id,
        new_device_id,
        is_virtual
    );

    tracing::info!(
        "🔍 BACKEND SWITCH: old_device_id={:?}, new_device_id={}, is_virtual={:?}",
        old_device_id,
        new_device_id,
        is_virtual
    );

    // Check if switching to the same device - no-op to prevent unnecessary stream restart
    if let Some(ref old_id) = old_device_id {
        if old_id == &new_device_id {
            tracing::info!(
                "📋 Device switch no-op: already using device {}",
                new_device_id
            );
            // Return the existing device configuration
            let existing_device = crate::entities::configured_audio_device::Entity::find()
                .filter(
                    crate::entities::configured_audio_device::Column::DeviceIdentifier
                        .eq(&new_device_id),
                )
                .one(audio_state.database.sea_orm())
                .await
                .map_err(|e| format!("Failed to query existing device: {}", e))?;
            return Ok(existing_device);
        }
    }

    // Query old device's channel number before removal (to preserve channel assignment)
    let old_channel_number = if let Some(ref old_id) = old_device_id {
        if !old_id.trim().is_empty() {
            // Get channel number from database before deleting
            match crate::commands::configurations::get_device_channel_number(&audio_state, old_id)
                .await
            {
                Ok(channel) => Some(channel),
                Err(e) => {
                    tracing::warn!(
                        "Failed to get channel number for old device '{}': {}",
                        old_id,
                        e
                    );
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Remove old device if specified
    if let Some(old_id) = old_device_id {
        if !old_id.trim().is_empty() {
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let remove_command =
                crate::audio::mixer::stream_management::AudioCommand::RemoveInputStream {
                    device_id: old_id.clone(),
                    response_tx,
                };

            if let Err(e) = audio_state.audio_command_tx.send(remove_command).await {
                let error_msg = format!(
                    "Audio system not available - failed to send remove command: {}",
                    e
                );
                tracing::error!("{}", error_msg);
                return Err(error_msg);
            }

            let _ = response_rx.await; // Don't fail on remove errors

            // Sync with database: remove old device configuration
            if let Err(e) =
                crate::commands::configurations::remove_device_configuration(&audio_state, &old_id)
                    .await
            {
                tracing::warn!(
                    "Failed to remove old device configuration from database: {}",
                    e
                );
                // Don't fail the command if database sync fails
            }
        }
    }

    let is_app_audio = is_virtual.unwrap_or(false);

    crate::commands::device_attachment::attach_input_device(
        &audio_state,
        &new_device_id,
        is_app_audio,
        old_channel_number, // Preserve channel assignment from old device
    )
    .await
}

#[tauri::command]
pub async fn safe_switch_output_device(
    audio_state: State<'_, AudioState>,
    old_device_id: Option<String>,
    new_device_id: String,
) -> Result<OutputDeviceSwitchResult, String> {
    log_command!(
        "safe_switch_output_device",
        "old: {:?}, new: {}",
        old_device_id,
        new_device_id
    );

    // Note: Duplicate output device detection is handled at client level in mixer store
    tracing::info!("🔊 Switching to output device: {}", new_device_id);

    // Re-pointing a destination at the device it already uses is a no-op rather
    // than a duplicate registration error.
    if old_device_id.as_deref() == Some(new_device_id.as_str()) {
        tracing::info!(
            "📋 Output device no-op: '{}' is already the destination",
            new_device_id
        );
        return Ok(divert_system_audio(&audio_state).await);
    }

    // Free the old destination first so its slot, and its hardware device, are
    // available to the new one.
    if let Some(old_id) = old_device_id.as_deref() {
        if !old_id.trim().is_empty() {
            remove_output_stream_internal(&audio_state, old_id).await?;
        }
    }

    // Get device handle using device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(&new_device_id, false) // false = output device
        .await
        .map_err(|e| {
            // A configured output that is simply unplugged lands here. Without a
            // log the mixer runs with zero outputs and no trace of why.
            let error_msg = format!("Failed to find output device {}: {}", new_device_id, e);
            tracing::error!(
                "{}: {} - the mix has no output until a reachable device is selected",
                "OUTPUT_UNAVAILABLE".on_red().bright_white(),
                error_msg
            );
            error_msg
        })?;

    // Extract device information for database sync and hog mode before consuming device_handle
    #[cfg(target_os = "macos")]
    let device_info = match &device_handle {
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => Some((
            coreaudio_device.name.clone(),
            coreaudio_device.sample_rate,
            coreaudio_device.channels,
        )),
        crate::audio::types::AudioDeviceHandle::ApplicationAudio(_) => {
            return Err(
                "Application audio devices are input-only and cannot be used as outputs"
                    .to_string(),
            );
        }
    };

    #[cfg(not(target_os = "macos"))]
    let device_info = None;

    // Create command based on device type
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
            crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioOutputStream {
                device_id: new_device_id.clone(),
                coreaudio_device,
                response_tx,
            }
        }
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::ApplicationAudio(_) => {
            return Err(
                "Application audio devices are input-only and cannot be used as outputs"
                    .to_string(),
            );
        }
        #[cfg(not(target_os = "macos"))]
        _ => return Err("Unsupported device type for this platform".to_string()),
    };

    // Send command to isolated audio thread
    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        let error_msg = format!("Audio system not available - failed to send command: {}", e);
        tracing::error!("{}", error_msg);
        return Err(error_msg);
    }

    // Wait for response from isolated audio thread
    match response_rx.await {
        Ok(Ok(())) => {
            // Sync with database: create new device configuration
            if let Some((device_name, sample_rate, channels)) = device_info {
                match crate::commands::configurations::create_device_configuration(
                    &audio_state,
                    &new_device_id,
                    &device_name,
                    sample_rate as i32,
                    channels as u32,
                    false, // is_input
                    false, // is_virtual
                    None,  // channel_number (outputs don't use channel numbers)
                )
                .await
                {
                    Ok(_) => {
                        // Device configuration created successfully
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create output device configuration in database: {}",
                            e
                        );
                    }
                }
            }

            Ok(divert_system_audio(&audio_state).await)
        }
        Ok(Err(e)) => {
            tracing::error!(
                "{}: Audio pipeline rejected output device '{}': {}",
                "OUTPUT_UNAVAILABLE".on_red().bright_white(),
                new_device_id,
                e
            );
            Err(format!("Failed to set output device: {}", e))
        }
        Err(_) => {
            tracing::error!(
                "{}: Audio system did not respond while setting output device '{}'",
                "OUTPUT_UNAVAILABLE".on_red().bright_white(),
                new_device_id
            );
            Err("Audio system did not respond".to_string())
        }
    }
}

#[tauri::command]
pub async fn remove_input_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    log_command!("remove_input_stream", "device: {}", device_id);

    // Create command for removal
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let command = crate::audio::mixer::stream_management::AudioCommand::RemoveInputStream {
        device_id: device_id.clone(),
        response_tx,
    };

    // Send command to isolated audio thread
    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        let error_msg = format!("Audio system not available - failed to send command: {}", e);
        tracing::error!("{}", error_msg);
        return Err(error_msg);
    }

    // Wait for response from isolated audio thread
    match response_rx.await {
        Ok(Ok(_)) => {
            println!(
                "✅ Successfully removed input stream via direct command: {}",
                device_id
            );

            // Sync with database: remove configured_audio_device entry
            if let Err(e) = crate::commands::configurations::remove_device_configuration(
                &audio_state,
                &device_id,
            )
            .await
            {
                tracing::warn!("Failed to remove device configuration from database: {}", e);
                // Don't fail the command if database sync fails
            }

            Ok(())
        }
        Ok(Err(e)) => Err(format!("Failed to remove input stream: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

/// Tear down every device registered by the current session
///
/// Called before restoring a different session so its devices register against a
/// clean pipeline. Recording and Icecast output taps are left running.
#[tauri::command]
pub async fn clear_session_devices(audio_state: State<'_, AudioState>) -> Result<usize, String> {
    log_command!("clear_session_devices");

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let command =
        crate::audio::mixer::stream_management::AudioCommand::ClearSessionDevices { response_tx };

    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        let error_msg = format!("Audio system not available - failed to send command: {}", e);
        tracing::error!("{}", error_msg);
        return Err(error_msg);
    }

    match response_rx.await {
        Ok(Ok(removed)) => {
            tracing::info!(
                "{} Cleared {} session devices",
                "SESSION_DEVICES_CLEARED".bright_green(),
                removed
            );
            Ok(removed)
        }
        Ok(Err(e)) => Err(format!("Failed to clear session devices: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

#[tauri::command]
pub async fn set_output_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    log_command!("set_output_stream", "device: {}", device_id);
    if device_id.trim().is_empty() {
        return Err("Device ID cannot be empty".to_string());
    }
    if device_id.len() > 256 {
        return Err("Device ID too long".to_string());
    }

    // **STREAMLINED ARCHITECTURE**: Bypass VirtualMixer and send command directly to IsolatedAudioManager
    println!(
        "🔊 Setting output stream directly via AudioCommand: {}",
        device_id
    );

    // Get device handle using device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(&device_id, false) // false = output device
        .await
        .map_err(|e| format!("Failed to find output device {}: {}", device_id, e))?;

    // Create command based on device type
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
            crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioOutputStream {
                device_id: device_id.clone(),
                coreaudio_device,
                response_tx,
            }
        }
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::ApplicationAudio(_) => {
            return Err(
                "Application audio devices are input-only and cannot be used as outputs"
                    .to_string(),
            );
        }
        #[cfg(not(target_os = "macos"))]
        _ => return Err("Unsupported device type for this platform".to_string()),
    };

    // Send command to isolated audio thread
    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        let error_msg = format!("Audio system not available - failed to send command: {}", e);
        tracing::error!("{}", error_msg);
        return Err(error_msg);
    }

    // Wait for response from isolated audio thread
    match response_rx.await {
        Ok(Ok(())) => {
            println!(
                "✅ Successfully set output stream via direct command: {}",
                device_id
            );
            Ok(())
        }
        Ok(Err(e)) => Err(format!("Failed to set output stream: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

// Device monitoring commands
#[tauri::command]
pub async fn start_device_monitoring(audio_state: State<'_, AudioState>) -> Result<String, String> {
    let mixer_guard = audio_state.mixer.lock().await;

    if mixer_guard.is_some() {
        // For now, just return success. The actual device monitoring implementation
        // needs refactoring to work with the app's mixer storage pattern.
        // This is a placeholder until we can properly integrate it.
        println!("✅ Device monitoring started (placeholder implementation)");
        Ok("Device monitoring started successfully (placeholder)".to_string())
    } else {
        Err("No mixer created - cannot start device monitoring".to_string())
    }
}

#[tauri::command]
pub async fn get_device_monitoring_stats() -> Result<Option<crate::DeviceMonitorStats>, String> {
    use crate::get_monitoring_stats_impl;
    Ok(get_monitoring_stats_impl().await)
}
