use crate::{AudioDeviceInfo, AudioState};
use tauri::State;

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
    let device_manager = audio_state.device_manager.lock().await;
    Ok(device_manager.get_device(&device_id).await)
}

// Device health monitoring commands
#[tauri::command]
pub async fn get_device_health(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    Ok(())
    // let mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mixer) = *mixer_guard {
    //     Ok(mixer.get_device_health_status(&device_id).await)
    // } else {
    //     Err("No mixer created".to_string())
    // }
}

#[tauri::command]
pub async fn get_all_device_health(
    audio_state: State<'_, AudioState>,
) -> Result<std::collections::HashMap<String, crate::audio::devices::DeviceHealth>, String> {
    // TODO: return fake Hashmap
    let mut health_map = std::collections::HashMap::new();
    health_map.insert(
        "fake_device".to_string(),
        crate::audio::devices::DeviceHealth::new_healthy("".to_string(), "".to_string()),
    );
    Ok(health_map)
}

#[tauri::command]
pub async fn report_device_error(
    audio_state: State<'_, AudioState>,
    device_id: String,
    error: String,
) -> Result<(), String> {
    Ok(())
    // let mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mixer) = *mixer_guard {
    //     mixer
    //         .audio_device_manager
    //         .report_device_error(&device_id, error)
    //         .await;
    //     Ok(())
    // } else {
    //     Err("No mixer created".to_string())
    // }
}

// Device switching commands
#[tauri::command]
pub async fn safe_switch_input_device(
    audio_state: State<'_, AudioState>,
    old_device_id: Option<String>,
    new_device_id: String,
    is_virtual: Option<bool>,
) -> Result<(), String> {
    // Check if switching to the same device - no-op to prevent unnecessary stream restart
    if let Some(ref old_id) = old_device_id {
        if old_id == &new_device_id {
            tracing::info!(
                "📋 Device switch no-op: already using device {}",
                new_device_id
            );
            return Ok(());
        }
    }

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

    // Get device handle - either from device manager or create application audio handle
    let (device_handle, device_info) = if is_app_audio {
        #[cfg(target_os = "macos")]
        {
            // Parse PID from device_id (format: "app-{pid}")
            let pid: u32 = new_device_id
                .strip_prefix("app-")
                .ok_or_else(|| format!("Invalid application audio device ID: {}", new_device_id))?
                .parse()
                .map_err(|e| {
                    format!(
                        "Failed to parse PID from device ID {}: {}",
                        new_device_id, e
                    )
                })?;

            // Get application info from ApplicationAudioManager
            let app_manager = audio_state.app_audio_manager.lock().await;
            let available_apps = app_manager
                .get_available_applications()
                .await
                .map_err(|e| format!("Failed to get available applications: {}", e))?;

            let app_info = available_apps
                .iter()
                .find(|app| app.pid == pid)
                .ok_or_else(|| format!("Application with PID {} not found", pid))?;

            let device_handle = crate::audio::types::AudioDeviceHandle::ApplicationAudio(
                crate::audio::types::ApplicationAudioDevice {
                    pid,
                    name: app_info.name.clone(),
                    sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
                    channels: 2,
                },
            );

            let info = (
                app_info.name.clone(),
                crate::types::DEFAULT_SAMPLE_RATE,
                2u16,
            );
            (device_handle, Some(info))
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Err("Application audio not supported on this platform".to_string());
        }
    } else {
        // Standard CoreAudio device
        let device_manager = audio_state.device_manager.lock().await;
        let device_handle = device_manager
            .find_audio_device(&new_device_id, true) // true = input device
            .await
            .map_err(|e| format!("Failed to find input device {}: {}", new_device_id, e))?;

        // Extract device information for database sync before consuming device_handle
        let device_info = match &device_handle {
            #[cfg(target_os = "macos")]
            crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => Some((
                coreaudio_device.name.clone(),
                coreaudio_device.sample_rate,
                coreaudio_device.channels,
            )),
            #[cfg(not(target_os = "macos"))]
            _ => None,
            #[cfg(target_os = "macos")]
            crate::audio::types::AudioDeviceHandle::ApplicationAudio(_) => None,
        };
        (device_handle, device_info)
    };

    // **FIX**: Create database entry BEFORE sending command
    // The audio pipeline needs to query the database for channel number during device setup
    if let Some((device_name, sample_rate, channels)) = device_info.clone() {
        if let Err(e) = crate::commands::configurations::create_device_configuration(
            &audio_state,
            &new_device_id,
            &device_name,
            sample_rate as i32,
            channels as u32,
            true, // is_input
            is_app_audio
                || new_device_id.contains("BlackHole")
                || new_device_id.contains("SoundflowerBed"),
        )
        .await
        {
            return Err(format!(
                "Failed to create device configuration in database: {}",
                e
            ));
        }
    }

    // Create command based on device type
    let buffer_capacity = 96000;
    let (producer, _consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
            crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioInputStream {
                device_id: new_device_id.clone(),
                coreaudio_device_id: coreaudio_device.device_id,
                device_name: coreaudio_device.name.clone(),
                channels: coreaudio_device.channels,
                producer,
                response_tx,
            }
        }
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::ApplicationAudio(app_device) => {
            crate::audio::mixer::stream_management::AudioCommand::AddApplicationAudioInputStream {
                device_id: new_device_id.clone(),
                pid: app_device.pid,
                device_name: app_device.name.clone(),
                channels: app_device.channels,
                producer,
                response_tx,
            }
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
            tracing::info!("✅ Successfully added input device: {}", new_device_id);
            Ok(())
        }
        Ok(Err(e)) => {
            // If audio pipeline fails, clean up the database entry we created
            tracing::error!("Failed to add input device to audio pipeline: {}", e);
            if let Err(cleanup_err) = crate::commands::configurations::remove_device_configuration(
                &audio_state,
                &new_device_id,
            )
            .await
            {
                tracing::warn!(
                    "Failed to clean up device configuration after error: {}",
                    cleanup_err
                );
            }
            Err(format!("Failed to add input device: {}", e))
        }
        Err(_) => {
            // If audio system doesn't respond, clean up the database entry
            if let Err(cleanup_err) = crate::commands::configurations::remove_device_configuration(
                &audio_state,
                &new_device_id,
            )
            .await
            {
                tracing::warn!(
                    "Failed to clean up device configuration after timeout: {}",
                    cleanup_err
                );
            }
            Err("Audio system did not respond".to_string())
        }
    }
}

#[tauri::command]
pub async fn safe_switch_output_device(
    audio_state: State<'_, AudioState>,
    new_device_id: String,
) -> Result<(), String> {
    // Note: Duplicate output device detection is handled at client level in mixer store
    tracing::info!("🔊 Switching to output device: {}", new_device_id);

    // Get device handle using device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(&new_device_id, false) // false = output device
        .await
        .map_err(|e| format!("Failed to find output device {}: {}", new_device_id, e))?;

    // Extract device information for database sync before consuming device_handle
    let device_info = match &device_handle {
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => Some((
            coreaudio_device.name.clone(),
            coreaudio_device.sample_rate,
            coreaudio_device.channels,
        )),
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::ApplicationAudio(_) => {
            return Err(
                "Application audio devices are input-only and cannot be used as outputs"
                    .to_string(),
            );
        }
        #[cfg(not(target_os = "macos"))]
        _ => None,
    };

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
                if let Err(e) = crate::commands::configurations::create_device_configuration(
                    &audio_state,
                    &new_device_id,
                    &device_name,
                    sample_rate as i32,
                    channels as u32,
                    false, // is_input
                    false, // is_virtual
                )
                .await
                {
                    tracing::warn!(
                        "Failed to create output device configuration in database: {}",
                        e
                    );
                }
            }
            Ok(())
        }
        Ok(Err(e)) => Err(format!("Failed to set output device: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

#[tauri::command]
pub async fn remove_input_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    // **STREAMLINED ARCHITECTURE**: Bypass VirtualMixer and send command directly to IsolatedAudioManager
    println!(
        "🗑️ Removing input stream directly via AudioCommand: {}",
        device_id
    );

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

#[tauri::command]
pub async fn set_output_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    // **CRASH FIX**: Validate device ID
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

#[tauri::command]
pub async fn remove_output_device(
    audio_state: State<'_, crate::AudioState>,
    device_id: String,
) -> Result<(), String> {
    // let mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mixer) = *mixer_guard {
    //     mixer
    //         .remove_output_device(&device_id)
    //         .await
    //         .map_err(|e| e.to_string())?;
    //     println!("✅ Removed output device via Tauri command: {}", device_id);
    // } else {
    //     return Err("No mixer created".to_string());
    // }
    Ok(())
}

#[tauri::command]
pub async fn update_output_device(
    audio_state: State<'_, crate::AudioState>,
    device_id: String,
    device_name: Option<String>,
    gain: Option<f32>,
    enabled: Option<bool>,
    is_monitor: Option<bool>,
) -> Result<(), String> {
    // let mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mixer) = *mixer_guard {
    //     // Get current device configuration
    //     let current_config = mixer.get_output_device(&device_id).await;

    //     if let Some(mut updated_device) = current_config {
    //         // Update specified fields
    //         if let Some(name) = device_name {
    //             updated_device.device_name = name;
    //         }
    //         if let Some(g) = gain {
    //             updated_device.gain = g;
    //         }
    //         if let Some(e) = enabled {
    //             updated_device.enabled = e;
    //         }
    //         if let Some(m) = is_monitor {
    //             updated_device.is_monitor = m;
    //         }

    //         mixer
    //             .update_output_device(&device_id, updated_device)
    //             .await
    //             .map_err(|e| e.to_string())?;
    //         println!("✅ Updated output device via Tauri command: {}", device_id);
    //     } else {
    //         return Err(format!("Output device not found: {}", device_id));
    //     }
    // } else {
    //     return Err("No mixer created".to_string());
    // }
    Ok(())
}
// CoreAudio specific commands
#[tauri::command]
pub async fn enumerate_coreaudio_devices(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<AudioDeviceInfo>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    let all_devices = device_manager
        .enumerate_devices()
        .await
        .map_err(|e| e.to_string())?;

    // Filter to only CoreAudio devices
    let coreaudio_devices: Vec<AudioDeviceInfo> = all_devices
        .into_iter()
        .filter(|device| device.host_api == "CoreAudio (Direct)")
        .collect();

    Ok(coreaudio_devices)
}

#[tauri::command]
pub async fn get_device_type_info(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<Option<String>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    if let Some(device_info) = device_manager.get_device(&device_id).await {
        Ok(Some(device_info.host_api))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn is_coreaudio_device(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<bool, String> {
    let device_manager = audio_state.device_manager.lock().await;
    if let Some(device_info) = device_manager.get_device(&device_id).await {
        Ok(device_info.host_api == "CoreAudio (Direct)")
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn enumerate_application_audio_devices(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<crate::audio::tap::ProcessInfo>, String> {
    #[cfg(target_os = "macos")]
    {
        let app_manager = audio_state.app_audio_manager.lock().await;
        app_manager
            .get_available_applications()
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}
