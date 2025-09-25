use crate::{AudioState, ChannelConfig, DeviceMonitorStats, VULevelData};
use tauri::State;

// Debug control commands
#[tauri::command]
pub fn set_debug_log_config(audio: bool, device: bool) {
    crate::log::set_debug_levels(crate::log::DebugLoggingConfig { audio, device });
}

#[tauri::command]
pub fn get_debug_log_config() -> crate::log::DebugLoggingConfig {
    crate::log::get_debug_levels()
}

// TODO: SQLite-based VU meter commands - needs implementation with new schema
// #[tauri::command]
// pub async fn get_recent_vu_levels(
//     audio_state: State<'_, AudioState>,
//     channel_id: u32,
//     limit: Option<i64>,
// ) -> Result<Vec<VULevelData>, String> {
//     let limit = limit.unwrap_or(50); // Default to last 50 readings
//     // TODO: Implement with new VULevelData::get_recent_for_device method
//     Err("Not implemented".to_string())
// }

// TODO: Master levels - removed from new schema
// #[tauri::command]
// pub async fn get_recent_master_levels(
//     audio_state: State<'_, AudioState>,
//     limit: Option<i64>,
// ) -> Result<Vec<MasterLevelData>, String> {
//     let limit = limit.unwrap_or(50); // Default to last 50 readings
//     // Master levels are now tracked per-device in VULevelData
//     Err("Deprecated - use device-specific VU levels".to_string())
// }

// TODO: Channel config - needs implementation with new schema
// #[tauri::command]
// pub async fn save_channel_config(
//     audio_state: State<'_, AudioState>,
//     channel: ChannelConfig,
// ) -> Result<u32, String> {
//     // TODO: Implement using AudioEffectsDefault and AudioEffectsCustom
//     Err("Not implemented".to_string())
// }

// TODO: Load channel configs - needs implementation with new schema
// #[tauri::command]
// pub async fn load_channel_configs(
//     audio_state: State<'_, AudioState>,
// ) -> Result<Vec<ChannelConfig>, String> {
//     // TODO: Implement using AudioEffectsDefault and AudioEffectsCustom
//     Err("Not implemented".to_string())
// }

#[tauri::command]
pub async fn cleanup_old_levels(audio_state: State<'_, AudioState>) -> Result<u64, String> {
    audio_state
        .database
        .cleanup_old_vu_levels()
        .await
        .map_err(|e| e.to_string())
}

// Simple commands
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
