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

/// What an output device is doing underneath the mixer
///
/// A device carries its own mute and volume and both sit below every stream
/// playing through it, so a perfectly running output can be silent with nothing
/// on screen to say why. This is how to ask.
#[tauri::command]
pub async fn get_output_health(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<crate::audio::devices::output_health::OutputHealth, String> {
    #[cfg(target_os = "macos")]
    {
        let manager = audio_state.device_manager.lock().await;

        let handle = manager
            .find_audio_device(&device_id, false)
            .await
            .map_err(|e| format!("Could not find output '{}': {}", device_id, e))?;

        let crate::audio::types::AudioDeviceHandle::CoreAudio(device) = handle else {
            return Err(format!("'{}' is not a CoreAudio output", device_id));
        };

        Ok(crate::audio::devices::output_health::output_health(
            device.device_id,
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (audio_state, device_id);
        Err("Output health is only available on macOS".to_string())
    }
}
