use crate::AudioState;
use colored::Colorize;
use tauri::State;
use tracing::{error, info};

#[tauri::command]
pub async fn enable_system_audio_capture(audio_state: State<'_, AudioState>) -> Result<(), String> {
    info!(
        "{} Enabling system audio capture",
        "SYSTEM_AUDIO_ENABLE".bright_cyan()
    );

    // Divert system audio to virtual device
    let mut router = audio_state.system_audio_router.lock().await;
    router
        .divert_system_audio_to_virtual_device()
        .await
        .map_err(|e| {
            error!(
                "{} Failed to divert system audio: {}",
                "SYSTEM_AUDIO_ERROR".bright_red(),
                e
            );
            format!("Failed to enable system audio capture: {}", e)
        })?;

    info!(
        "{} System audio capture enabled - audio now routed through virtual device",
        "SYSTEM_AUDIO_ENABLED".bright_green()
    );

    // TODO: Add virtual device as input to mixer
    // This will capture from the virtual device's input stream and mix it with other inputs

    Ok(())
}

#[tauri::command]
pub async fn disable_system_audio_capture(
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    info!(
        "{} Disabling system audio capture",
        "SYSTEM_AUDIO_DISABLE".bright_cyan()
    );

    let mut router = audio_state.system_audio_router.lock().await;
    router.restore_original_default().await.map_err(|e| {
        error!(
            "{} Failed to restore system audio: {}",
            "SYSTEM_AUDIO_ERROR".bright_red(),
            e
        );
        format!("Failed to disable system audio capture: {}", e)
    })?;

    info!(
        "{} System audio capture disabled - audio restored to original output",
        "SYSTEM_AUDIO_DISABLED".bright_green()
    );

    // TODO: Remove virtual device input from mixer

    Ok(())
}

#[tauri::command]
pub async fn get_system_audio_status(audio_state: State<'_, AudioState>) -> Result<bool, String> {
    use crate::db::SystemAudioStateService;

    let db = audio_state.database.sea_orm();
    let state = SystemAudioStateService::get_or_create(db)
        .await
        .map_err(|e| format!("Failed to get system audio state: {}", e))?;

    Ok(state.is_diverted)
}
