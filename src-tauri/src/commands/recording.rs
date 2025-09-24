use crate::audio::recording::types::{MetadataPresets, RecordingPresets};
use crate::audio::recording::{
    RecordingConfig, RecordingHistoryEntry, RecordingMetadata, RecordingStatus,
};
use crate::{AudioState, RecordingState};
use tauri::State;

// ================================================================================================
// RECORDING SERVICE COMMANDS
// ================================================================================================

#[tauri::command]
pub async fn start_recording(
    recording_state: State<'_, RecordingState>,
    audio_state: State<'_, AudioState>,
    config: RecordingConfig,
) -> Result<String, String> {
    println!("🎙️ Starting recording with config: {}", config.name);

    // Step 1: Send command to IsolatedAudioManager to create recording OutputWorker
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let command = crate::audio::mixer::stream_management::AudioCommand::StartRecording {
        session_id: config.id.clone(),
        recording_config: config.clone(),
        response_tx,
    };

    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        return Err(format!("Failed to send recording start command: {}", e));
    }

    // Step 2: Wait for the RTRB consumer from the audio pipeline
    let recording_consumer = match response_rx.await {
        Ok(Ok(consumer)) => consumer,
        Ok(Err(e)) => return Err(format!("Failed to create recording output worker: {}", e)),
        Err(e) => return Err(format!("Failed to receive response: {}", e)),
    };

    println!("🔄 Received RTRB consumer from audio pipeline");

    // Step 3: Start recording service with the RTRB consumer
    match recording_state
        .service
        .start_recording(config.clone(), recording_consumer)
        .await
    {
        Ok(session_id) => {
            println!("✅ Recording started with session ID: {}", session_id);
            Ok(session_id)
        }
        Err(e) => {
            println!("❌ Failed to start recording service: {}", e);

            // **CLEANUP**: Recording service failed, need to clean up the OutputWorker
            println!("🧹 Cleaning up OutputWorker after recording service failure...");
            let (cleanup_tx, cleanup_rx) = tokio::sync::oneshot::channel();
            let cleanup_command = crate::audio::mixer::stream_management::AudioCommand::StopRecording {
                session_id: config.id.clone(),
                response_tx: cleanup_tx,
            };

            if let Err(cleanup_err) = audio_state.audio_command_tx.send(cleanup_command).await {
                println!("⚠️ Failed to send cleanup command: {}", cleanup_err);
            } else {
                match cleanup_rx.await {
                    Ok(Ok(())) => println!("✅ OutputWorker cleaned up successfully"),
                    Ok(Err(cleanup_err)) => println!("⚠️ OutputWorker cleanup failed: {}", cleanup_err),
                    Err(cleanup_err) => println!("⚠️ Failed to receive cleanup response: {}", cleanup_err),
                }
            }

            Err(format!("Failed to start recording: {}", e))
        }
    }
}

#[tauri::command]
pub async fn stop_recording(
    recording_state: State<'_, RecordingState>,
    audio_state: State<'_, AudioState>,
) -> Result<Option<RecordingHistoryEntry>, String> {
    println!("🛑 Stopping recording...");

    // Step 1: Stop the recording service first to cleanly close files
    let history_entry = match recording_state.service.stop_recording().await {
        Ok(entry) => {
            if let Some(ref entry) = entry {
                println!("✅ Recording service stopped: {:?}", entry.file_path);
            } else {
                println!("⚠️ No active recording to stop in service");
            }
            entry
        }
        Err(e) => {
            println!("❌ Failed to stop recording service: {}", e);
            return Err(format!("Failed to stop recording: {}", e));
        }
    };

    // Step 2: Send command to IsolatedAudioManager to clean up the output worker
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let command = crate::audio::mixer::stream_management::AudioCommand::StopRecording {
        session_id: "current".to_string(), // Session ID doesn't matter since there's only one recording
        response_tx,
    };

    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        println!("⚠️ Failed to send recording stop command (recording already stopped cleanly): {}", e);
        // Don't return error here - recording service already stopped successfully
    } else {
        // Wait for cleanup completion
        match response_rx.await {
            Ok(Ok(())) => println!("✅ Recording output worker cleaned up successfully"),
            Ok(Err(e)) => println!("⚠️ Recording output worker cleanup failed: {}", e),
            Err(e) => println!("⚠️ Failed to receive cleanup response: {}", e),
        }
    }

    Ok(history_entry)
}

#[tauri::command]
pub async fn get_recording_status(
    recording_state: State<'_, RecordingState>,
) -> Result<RecordingStatus, String> {
    let status = recording_state.service.get_status().await;
    if status.is_recording {
        println!(
            "🔍 get_recording_status API called - is_recording: {}, session: {:?}",
            status.is_recording,
            status
                .session
                .as_ref()
                .map(|s| format!("{}s, {}B", s.duration_seconds, s.file_size_bytes))
        );
    }
    Ok(status)
}

#[tauri::command]
pub async fn save_recording_config(
    recording_state: State<'_, RecordingState>,
    config: RecordingConfig,
) -> Result<String, String> {
    println!("💾 Saving recording config: {}", config.name);

    match recording_state.service.save_config(config.clone()).await {
        Ok(()) => {
            println!("✅ Recording config saved: {}", config.name);
            Ok(format!("Config '{}' saved successfully", config.name))
        }
        Err(e) => {
            eprintln!("❌ Failed to save recording config: {}", e);
            Err(format!("Failed to save config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_recording_configs(
    recording_state: State<'_, RecordingState>,
) -> Result<Vec<RecordingConfig>, String> {
    Ok(recording_state.service.get_configs().await)
}

#[tauri::command]
pub async fn get_recording_history(
    recording_state: State<'_, RecordingState>,
) -> Result<Vec<RecordingHistoryEntry>, String> {
    Ok(recording_state.service.get_history().await)
}

#[tauri::command]
pub async fn create_default_recording_config() -> Result<RecordingConfig, String> {
    Ok(RecordingConfig::default())
}

#[tauri::command]
pub async fn select_recording_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use std::sync::{Arc, Mutex};
    use tauri_plugin_dialog::DialogExt;
    use tokio::time::Duration;

    println!("🔍 select_recording_directory command called");

    // Use a shared result to capture the callback result
    let result: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    // Show directory picker dialog with callback
    app.dialog().file().pick_folder(move |folder_path| {
        let path_result = if let Some(path) = folder_path {
            let path_str = path.to_string();
            println!("📁 User selected directory: {}", path_str);
            Some(path_str)
        } else {
            println!("📁 User cancelled directory selection");
            None
        };

        // Store the result
        if let Ok(mut guard) = result_clone.lock() {
            *guard = Some(path_result);
        }
    });

    // Wait for the dialog result with timeout
    let timeout_duration = Duration::from_secs(30); // 30 second timeout
    let start_time = std::time::Instant::now();

    loop {
        if start_time.elapsed() > timeout_duration {
            return Err("Dialog timeout".to_string());
        }

        if let Ok(guard) = result.lock() {
            if let Some(path_result) = guard.as_ref() {
                return Ok(path_result.clone());
            }
        }

        // Small delay to avoid busy waiting
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// ================================================================================================
// METADATA AND PRESET COMMANDS
// ================================================================================================

#[tauri::command]
pub async fn get_metadata_presets() -> Result<Vec<(String, RecordingMetadata)>, String> {
    Ok(MetadataPresets::get_all_presets()
        .into_iter()
        .map(|(name, metadata)| (name.to_string(), metadata))
        .collect())
}

#[tauri::command]
pub async fn get_recording_presets() -> Result<Vec<RecordingConfig>, String> {
    Ok(RecordingPresets::get_all_presets()
        .into_iter()
        .map(|(_, config)| config)
        .collect())
}

#[tauri::command]
pub async fn update_recording_metadata(
    recording_state: State<'_, RecordingState>,
    metadata: RecordingMetadata,
) -> Result<(), String> {
    println!(
        "📝 Updating session metadata with {} fields",
        metadata.get_display_fields().len()
    );

    match recording_state
        .service
        .update_session_metadata(metadata)
        .await
    {
        Ok(()) => {
            println!("✅ Session metadata updated successfully");
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Failed to update session metadata: {}", e);
            Err(format!("Failed to update metadata: {}", e))
        }
    }
}
