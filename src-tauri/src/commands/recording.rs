use tauri::State;
use crate::{AudioState, RecordingState, recording_service::{RecordingConfig, RecordingStatus, RecordingHistoryEntry}};

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
    
    // Get audio output receiver from mixer
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        let audio_rx = mixer.get_audio_output_receiver();
        
        match recording_state.service.start_recording(config, audio_rx).await {
            Ok(session_id) => {
                println!("✅ Recording started with session ID: {}", session_id);
                Ok(session_id)
            }
            Err(e) => {
                eprintln!("❌ Failed to start recording: {}", e);
                Err(format!("Failed to start recording: {}", e))
            }
        }
    } else {
        Err("No mixer available - please create mixer first".to_string())
    }
}

#[tauri::command]
pub async fn stop_recording(
    recording_state: State<'_, RecordingState>,
) -> Result<Option<RecordingHistoryEntry>, String> {
    println!("🛑 Stopping recording...");
    
    match recording_state.service.stop_recording().await {
        Ok(history_entry) => {
            if let Some(ref entry) = history_entry {
                println!("✅ Recording stopped: {:?}", entry.file_path);
            } else {
                println!("⚠️ No active recording to stop");
            }
            Ok(history_entry)
        }
        Err(e) => {
            eprintln!("❌ Failed to stop recording: {}", e);
            Err(format!("Failed to stop recording: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_recording_status(
    recording_state: State<'_, RecordingState>,
) -> Result<RecordingStatus, String> {
    Ok(recording_state.service.get_status().await)
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
    use tauri_plugin_dialog::DialogExt;
    use std::sync::{Arc, Mutex};
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