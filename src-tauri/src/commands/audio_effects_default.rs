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

    // Look up the device identifier from the configured device
    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateInputGain {
                device_id: device.device_identifier,
                gain,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

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

    // Look up the device identifier from the configured device
    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateInputPan {
                device_id: device.device_identifier,
                pan,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    AudioEffectsDefaultService::update_pan(state.database.sea_orm(), &effects_id, pan)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Switch a channel's effects chain on or off, in the engine and on its row.
///
/// The switch persists with the settings it gates, so a relaunched channel
/// comes back with its chain the way it was left — on and processing, or off
/// and free.
#[tauri::command]
pub async fn update_audio_effects_default_effects_enabled(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    enabled: bool,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Effects {} for device {}",
        "UPDATE_EFFECTS_ENABLED".on_yellow().purple(),
        if enabled { "enabled" } else { "disabled" },
        device_id
    );

    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateInputEffectsEnabled {
                device_id: device.device_identifier,
                enabled,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    AudioEffectsDefaultService::update_effects_enabled(
        state.database.sea_orm(),
        &effects_id,
        enabled,
    )
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

    // Look up the device identifier from the configured device
    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateInputMuted {
                device_id: device.device_identifier,
                muted,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

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

    // Look up the device identifier from the configured device
    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateInputSolo {
                device_id: device.device_identifier,
                solo,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

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

/// Update a channel's EQ in the engine and on its effects row.
///
/// Any band left as None keeps its current value, so a single knob drag sends
/// only what moved.
#[tauri::command]
pub async fn update_audio_effects_default_eq(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    low_gain: Option<f32>,
    mid_gain: Option<f32>,
    high_gain: Option<f32>,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Updating EQ for device {} (low: {:?}, mid: {:?}, high: {:?})",
        "UPDATE_EQ".on_yellow().purple(),
        device_id,
        low_gain,
        mid_gain,
        high_gain
    );

    // Look up the device identifier from the configured device
    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateChannelEq {
                device_id: device.device_identifier,
                low_db: low_gain,
                mid_db: mid_gain,
                high_db: high_gain,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    AudioEffectsDefaultService::update_eq(
        state.database.sea_orm(),
        &effects_id,
        low_gain,
        mid_gain,
        high_gain,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Update a channel's compressor in the engine and on its effects row.
#[tauri::command]
pub async fn update_audio_effects_default_compressor(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    threshold: Option<f32>,
    ratio: Option<f32>,
    attack: Option<f32>,
    release: Option<f32>,
    enabled: Option<bool>,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Updating compressor for device {}",
        "UPDATE_COMPRESSOR".on_yellow().purple(),
        device_id
    );

    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateChannelCompressor {
                device_id: device.device_identifier,
                threshold_db: threshold,
                ratio,
                attack_ms: attack,
                release_ms: release,
                enabled,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    AudioEffectsDefaultService::update_compressor(
        state.database.sea_orm(),
        &effects_id,
        threshold,
        ratio,
        attack,
        release,
        enabled,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Update a channel's limiter in the engine and on its effects row.
#[tauri::command]
pub async fn update_audio_effects_default_limiter(
    effects_id: String,
    device_id: String,
    configuration_id: String,
    threshold: Option<f32>,
    enabled: Option<bool>,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    tracing::info!(
        "{}: Updating limiter for device {}",
        "UPDATE_LIMITER".on_yellow().purple(),
        device_id
    );

    let device =
        crate::db::ConfiguredAudioDeviceService::get_by_id(state.database.sea_orm(), &device_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Configured device {} not found", device_id))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateChannelLimiter {
                device_id: device.device_identifier,
                threshold_db: threshold,
                enabled,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    AudioEffectsDefaultService::update_limiter(
        state.database.sea_orm(),
        &effects_id,
        threshold,
        enabled,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}
