use tauri::State;
use crate::{
    AudioState,
    audio::{FilePlayerConfig, PlaybackAction, PlaybackStatus, QueuedTrack},
};
use std::path::PathBuf;

// State for file player service
pub struct FilePlayerState {
    pub service: crate::audio::FilePlayerService,
}

// File player management commands
#[tauri::command]
pub async fn create_file_player(
    file_player_state: State<'_, FilePlayerState>,
    config: FilePlayerConfig,
) -> Result<String, String> {
    println!("🎵 Creating file player: {}", config.name);
    
    file_player_state.service
        .get_manager()
        .create_player(config)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_file_player(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
) -> Result<(), String> {
    println!("🗑️ Removing file player: {}", player_id);
    
    file_player_state.service
        .get_manager()
        .remove_player(&player_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_file_players(
    file_player_state: State<'_, FilePlayerState>,
) -> Result<Vec<(String, String)>, String> {
    Ok(file_player_state.service
        .get_manager()
        .list_players())
}

#[tauri::command]
pub async fn get_file_player_devices(
    file_player_state: State<'_, FilePlayerState>,
) -> Result<Vec<crate::AudioDeviceInfo>, String> {
    Ok(file_player_state.service
        .get_manager()
        .get_devices())
}

// Queue management commands
#[tauri::command]
pub async fn add_track_to_player(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
    file_path: String,
) -> Result<String, String> {
    println!("📀 Adding track to player {}: {}", player_id, file_path);
    
    let path = PathBuf::from(file_path);
    file_player_state.service
        .get_manager()
        .add_track_to_player(&player_id, path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_track_from_player(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
    track_id: String,
) -> Result<(), String> {
    println!("🗑️ Removing track {} from player {}", track_id, player_id);
    
    file_player_state.service
        .get_manager()
        .remove_track_from_player(&player_id, &track_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_player_queue(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
) -> Result<Vec<QueuedTrack>, String> {
    file_player_state.service
        .get_manager()
        .get_player_queue(&player_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_player_queue(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
) -> Result<(), String> {
    println!("🧹 Clearing queue for player: {}", player_id);
    
    file_player_state.service
        .get_manager()
        .clear_player_queue(&player_id)
        .map_err(|e| e.to_string())
}

// Playback control commands
#[tauri::command]
pub async fn control_file_player(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
    action: PlaybackAction,
) -> Result<(), String> {
    println!("🎮 Controlling player {}: {:?}", player_id, action);
    
    file_player_state.service
        .get_manager()
        .control_player(&player_id, action)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_player_status(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
) -> Result<PlaybackStatus, String> {
    file_player_state.service
        .get_manager()
        .get_player_status(&player_id)
        .map_err(|e| e.to_string())
}

// File system commands
#[tauri::command]
pub async fn browse_audio_files() -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    use std::sync::{Arc, Mutex};
    use tokio::time::Duration;
    
    println!("🔍 Opening audio file browser dialog");
    
    // For now, return error since we need to implement proper file dialog
    // TODO: Implement multi-file selection dialog
    Err("File browser not yet implemented".to_string())
}

#[tauri::command]
pub async fn get_supported_audio_formats() -> Result<Vec<String>, String> {
    Ok(vec![
        "mp3".to_string(),
        "flac".to_string(),
        "wav".to_string(),
        "ogg".to_string(),
        "m4a".to_string(),
        "aac".to_string(),
    ])
}

#[tauri::command]
pub async fn validate_audio_file(file_path: String) -> Result<bool, String> {
    let path = PathBuf::from(file_path);
    
    // Check if file exists
    if !path.exists() {
        return Ok(false);
    }
    
    // Check if it's a file (not directory)
    if !path.is_file() {
        return Ok(false);
    }
    
    // Check file extension
    if let Some(extension) = path.extension() {
        if let Some(ext_str) = extension.to_str() {
            let supported_formats = vec!["mp3", "flac", "wav", "ogg", "m4a", "aac"];
            return Ok(supported_formats.contains(&ext_str.to_lowercase().as_str()));
        }
    }
    
    Ok(false)
}