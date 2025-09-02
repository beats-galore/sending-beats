use tauri::State;
use crate::{AudioState, VULevelData, MasterLevelData, ChannelConfig, DeviceMonitorStats};

// Debug control commands
#[tauri::command]
pub fn set_audio_debug_enabled(enabled: bool) {
    crate::log::set_audio_debug(enabled);
}

#[tauri::command]
pub fn get_audio_debug_enabled() -> bool {
    crate::log::is_audio_debug_enabled()
}

// SQLite-based VU meter commands for improved performance
#[tauri::command]
pub async fn get_recent_vu_levels(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    limit: Option<i64>,
) -> Result<Vec<VULevelData>, String> {
    let limit = limit.unwrap_or(50); // Default to last 50 readings
    audio_state.database
        .get_recent_vu_levels(channel_id, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_master_levels(
    audio_state: State<'_, AudioState>,
    limit: Option<i64>,
) -> Result<Vec<MasterLevelData>, String> {
    let limit = limit.unwrap_or(50); // Default to last 50 readings
    audio_state.database
        .get_recent_master_levels(limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_channel_config(
    audio_state: State<'_, AudioState>,
    channel: ChannelConfig,
) -> Result<u32, String> {
    audio_state.database
        .save_channel_config(&channel)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_channel_configs(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<ChannelConfig>, String> {
    audio_state.database
        .load_channel_configs()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_old_levels(
    audio_state: State<'_, AudioState>,
) -> Result<u64, String> {
    audio_state.database
        .cleanup_old_vu_levels()
        .await
        .map_err(|e| e.to_string())
}

// Simple commands
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}