use crate::{AudioDeviceInfo, AudioState};
use tauri::State;
use cpal::traits::DeviceTrait;

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
) -> Result<Option<crate::audio::devices::DeviceHealth>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_device_health_status(&device_id).await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn get_all_device_health(
    audio_state: State<'_, AudioState>,
) -> Result<std::collections::HashMap<String, crate::audio::devices::DeviceHealth>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_all_device_health_statuses().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn report_device_error(
    audio_state: State<'_, AudioState>,
    device_id: String,
    error: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.audio_device_manager.report_device_error(&device_id, error).await;
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

// Device switching commands
#[tauri::command]
pub async fn safe_switch_input_device(
    audio_state: State<'_, AudioState>,
    old_device_id: Option<String>,
    new_device_id: String,
) -> Result<(), String> {
    // **CRASH FIX**: Validate input device ID
    if new_device_id.trim().is_empty() {
        return Err("Device ID cannot be empty".to_string());
    }
    if new_device_id.len() > 256 {
        return Err("Device ID too long".to_string());
    }

    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        println!(
            "🔄 Switching input device from {:?} to {}",
            old_device_id, new_device_id
        );

        // Remove old device if specified
        if let Some(old_id) = old_device_id {
            if !old_id.trim().is_empty() {
                println!("🗑️ Removing old input device: {}", old_id);
                if let Err(e) = mixer.remove_input_stream(&old_id).await {
                    eprintln!(
                        "Warning: Failed to remove old input device {}: {}",
                        old_id, e
                    );
                    // Continue anyway - don't fail the entire operation
                }
                // **CRASH FIX**: Add delay to allow cleanup
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        // **CRASH FIX**: Add new device with better error handling
        println!("➕ Adding new input device: {}", new_device_id);
        match mixer.add_input_stream(&new_device_id).await {
            Ok(()) => {
                println!(
                    "✅ Successfully switched input device to: {}",
                    new_device_id
                );
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to add input stream for {}: {}", new_device_id, e);
                Err(format!("Failed to add input device: {}", e))
            }
        }
    } else {
        eprintln!("❌ Cannot switch input device: No mixer has been created yet");
        Err("No mixer created - please create mixer first".to_string())
    }
}

#[tauri::command]
pub async fn safe_switch_output_device(
    audio_state: State<'_, AudioState>,
    new_device_id: String,
) -> Result<(), String> {
    // **CRASH FIX**: Validate output device ID
    if new_device_id.trim().is_empty() {
        return Err("Device ID cannot be empty".to_string());
    }
    if new_device_id.len() > 256 {
        return Err("Device ID too long".to_string());
    }

    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        println!("🔊 Switching output device to: {}", new_device_id);

        // **CRASH FIX**: Better error handling and logging
        match mixer.set_output_stream(&new_device_id).await {
            Ok(()) => {
                println!(
                    "✅ Successfully switched output device to: {}",
                    new_device_id
                );
                Ok(())
            }
            Err(e) => {
                eprintln!(
                    "❌ Failed to set output stream for {}: {}",
                    new_device_id, e
                );
                Err(format!("Failed to set output device: {}", e))
            }
        }
    } else {
        eprintln!("❌ Cannot switch output device: No mixer has been created yet");
        Err("No mixer created - please create mixer first".to_string())
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

    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // **CRASH FIX**: Use basic add_input_stream for better compatibility
        match mixer.add_input_stream(&device_id).await {
            Ok(()) => {
                println!("✅ Successfully added input stream: {}", device_id);
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to add input stream for {}: {}", device_id, e);
                Err(format!("Failed to add input stream: {}", e))
            }
        }
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn remove_input_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer
            .remove_input_stream(&device_id)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
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

    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        match mixer.set_output_stream(&device_id).await {
            Ok(()) => {
                println!("✅ Successfully set output stream: {}", device_id);
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to set output stream for {}: {}", device_id, e);
                Err(format!("Failed to set output stream: {}", e))
            }
        }
    } else {
        Err("No mixer created".to_string())
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
pub async fn stop_device_monitoring() -> Result<String, String> {
    use crate::stop_monitoring_impl;

    match stop_monitoring_impl().await {
        Ok(()) => {
            println!("✅ Device monitoring stopped");
            Ok("Device monitoring stopped successfully".to_string())
        }
        Err(e) => {
            eprintln!("❌ Failed to stop device monitoring: {}", e);
            Err(format!("Failed to stop device monitoring: {}", e))
        }
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
    
    // Get the actual CPAL device using the device manager
    let device_manager = audio_state.device_manager.lock().await;
    let device_handle = device_manager.find_audio_device(&device_id, false).await
        .map_err(|e| format!("Failed to find output device {}: {}", device_id, e))?;
    
    let device = match device_handle {
        crate::audio::types::AudioDeviceHandle::Cpal(cpal_device) => cpal_device,
        #[cfg(target_os = "macos")]
        _ => return Err("Only CPAL devices supported for output streams".to_string()),
        #[cfg(not(target_os = "macos"))]
        _ => return Err("Unknown device handle type".to_string()),
    };
    
    // Get the default output config for this device
    let config = device.default_output_config()
        .map_err(|e| format!("Failed to get device config: {}", e))?
        .config();
    
    // Send AddOutputStream command to isolated audio thread
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    
    let command = crate::audio::mixer::stream_management::AudioCommand::AddOutputStream {
        device_id: device_id.clone(),
        device,
        config,
        response_tx,
    };
    
    // Send command to isolated audio thread
    if let Err(_) = audio_state.audio_command_tx.send(command).await {
        return Err("Audio system not available".to_string());
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
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer
            .remove_output_device(&device_id)
            .await
            .map_err(|e| e.to_string())?;
        println!("✅ Removed output device via Tauri command: {}", device_id);
    } else {
        return Err("No mixer created".to_string());
    }
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
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // Get current device configuration
        let current_config = mixer.get_output_device(&device_id).await;

        if let Some(mut updated_device) = current_config {
            // Update specified fields
            if let Some(name) = device_name {
                updated_device.device_name = name;
            }
            if let Some(g) = gain {
                updated_device.gain = g;
            }
            if let Some(e) = enabled {
                updated_device.enabled = e;
            }
            if let Some(m) = is_monitor {
                updated_device.is_monitor = m;
            }

            mixer
                .update_output_device(&device_id, updated_device)
                .await
                .map_err(|e| e.to_string())?;
            println!("✅ Updated output device via Tauri command: {}", device_id);
        } else {
            return Err(format!("Output device not found: {}", device_id));
        }
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_output_devices(
    audio_state: State<'_, crate::AudioState>,
) -> Result<Vec<crate::audio::types::OutputDevice>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_output_devices().await)
    } else {
        Err("No mixer created".to_string())
    }
}
