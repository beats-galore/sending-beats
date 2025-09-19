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
) -> Result<(), String> {
    // Remove old device if specified
    if let Some(old_id) = old_device_id {
        if !old_id.trim().is_empty() {
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let remove_command =
                crate::audio::mixer::stream_management::AudioCommand::RemoveInputStream {
                    device_id: old_id,
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
        }
    }

    // Get device handle using device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(&new_device_id, true) // true = input device
        .await
        .map_err(|e| format!("Failed to find input device {}: {}", new_device_id, e))?;

    // Create command based on device type
    let buffer_capacity = 8192;
    let (producer, _consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
            crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioInputStream {
                device_id: new_device_id,
                coreaudio_device_id: coreaudio_device.device_id,
                device_name: coreaudio_device.name,
                channels: 2,
                producer,
                input_notifier: std::sync::Arc::new(tokio::sync::Notify::new()),
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
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(format!("Failed to add input device: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

#[tauri::command]
pub async fn safe_switch_output_device(
    audio_state: State<'_, AudioState>,
    new_device_id: String,
) -> Result<(), String> {
    // Get device handle using device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(&new_device_id, false) // false = output device
        .await
        .map_err(|e| format!("Failed to find output device {}: {}", new_device_id, e))?;

    // Create command based on device type
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
            crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioOutputStream {
                device_id: new_device_id,
                coreaudio_device,
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
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(format!("Failed to set output device: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

// Audio stream management commands
#[tauri::command]
pub async fn add_input_stream(
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
        "🎤 Adding input stream directly via AudioCommand: {}",
        device_id
    );

    // Get device handle using device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(&device_id, true) // true = input device
        .await
        .map_err(|e| format!("Failed to find input device {}: {}", device_id, e))?;

    // Create command based on device type
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
            // Create RTRB producer/consumer pair that IsolatedAudioManager expects
            // The IsolatedAudioManager will create its own pair internally, but we need to provide one for the command
            let buffer_capacity = 8192; // Default buffer size
            let (producer, _consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);

            crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioInputStream {
                device_id: device_id.clone(),
                coreaudio_device_id: coreaudio_device.device_id,
                device_name: coreaudio_device.name.clone(),
                channels: 2, // stereo
                producer,
                input_notifier: std::sync::Arc::new(tokio::sync::Notify::new()),
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
            println!(
                "✅ Successfully added input stream via direct command: {}",
                device_id
            );
            Ok(())
        }
        Ok(Err(e)) => Err(format!("Failed to add input stream: {}", e)),
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

// Multiple output device management commands
#[tauri::command]
pub async fn add_output_device(
    audio_state: State<'_, crate::AudioState>,
    device_id: String,
    device_name: String,
    gain: Option<f32>,
    is_monitor: Option<bool>,
) -> Result<(), String> {
    // NEW ARCHITECTURE: Use command queue instead of direct mixer access

    // Get the device handle using the device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(&device_id, false)
        .await
        .map_err(|e| format!("Failed to find output device {}: {}", device_id, e))?;

    // Send appropriate command based on device type
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
            println!("✅ Added output device via command queue: {}", device_id);
            Ok(())
        }
        Ok(Err(e)) => Err(format!("Failed to add output device: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
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
