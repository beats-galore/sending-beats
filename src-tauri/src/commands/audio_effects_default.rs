use crate::db::AudioEffectsDefaultService;
use crate::entities::audio_effects_default;
use crate::AudioState;
use colored::*;
use tauri::State;

#[tauri::command]
pub async fn get_audio_effects_defaults(
    configuration_id: String,
    state: State<'_, AudioState>,
) -> Result<Vec<audio_effects_default::Model>, String> {
    tracing::info!(
        "{}: Getting audio effects defaults for configuration: {}",
        "GET_DEFAULTS".on_yellow().purple(),
        configuration_id
    );

    AudioEffectsDefaultService::list_for_configuration(state.database.sea_orm(), &configuration_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_audio_effects_default_gain(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    gain: f32,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Updating gain for device {} to {}",
        "UPDATE_GAIN".on_yellow().purple(),
        device_id,
        gain
    );

    let mut pipeline = state.pipeline.lock().await;
    pipeline
        .update_input_gain(&device_id, gain)
        .map_err(|e| e.to_string())?;
    drop(pipeline);

    AudioEffectsDefaultService::update_gain(state.database.sea_orm(), &effects_id, gain)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_audio_effects_default_pan(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    pan: f32,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Updating pan for device {} to {}",
        "UPDATE_PAN".on_yellow().purple(),
        device_id,
        pan
    );

    let mut pipeline = state.pipeline.lock().await;
    pipeline
        .update_input_pan(&device_id, pan)
        .map_err(|e| e.to_string())?;
    drop(pipeline);

    AudioEffectsDefaultService::update_pan(state.database.sea_orm(), &effects_id, pan)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_audio_effects_default_mute(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    muted: bool,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Updating mute for device {} to {}",
        "UPDATE_MUTE".on_yellow().purple(),
        device_id,
        muted
    );

    let mut pipeline = state.pipeline.lock().await;
    pipeline
        .update_input_muted(&device_id, muted)
        .map_err(|e| e.to_string())?;
    drop(pipeline);

    AudioEffectsDefaultService::update_mute(state.database.sea_orm(), &effects_id, muted)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_audio_effects_default_solo(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    solo: bool,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Updating solo for device {} to {}",
        "UPDATE_SOLO".on_yellow().purple(),
        device_id,
        solo
    );

    let mut pipeline = state.pipeline.lock().await;
    pipeline
        .update_input_solo(&device_id, solo)
        .map_err(|e| e.to_string())?;
    drop(pipeline);

    AudioEffectsDefaultService::update_solo(
        state.database.sea_orm(),
        &configuration_id,
        &effects_id,
        solo,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}
