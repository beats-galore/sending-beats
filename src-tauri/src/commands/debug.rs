use crate::{AudioState, DeviceMonitorStats};
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
