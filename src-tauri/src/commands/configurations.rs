use crate::db::{
    audio_mixer_configurations::AudioMixerConfiguration,
    configured_audio_devices::ConfiguredAudioDevice,
};
use crate::AudioState;
use anyhow::Result;
use tauri::State;
use uuid::Uuid;

/// Get all reusable configurations
#[tauri::command]
pub async fn get_reusable_configurations(
    state: State<'_, AudioState>,
) -> Result<Vec<AudioMixerConfiguration>, String> {
    AudioMixerConfiguration::list_reusable(state.database.pool())
        .await
        .map_err(|e| e.to_string())
}

/// Get the currently active session configuration
#[tauri::command]
pub async fn get_active_session_configuration(
    state: State<'_, AudioState>,
) -> Result<Option<AudioMixerConfiguration>, String> {
    AudioMixerConfiguration::get_active_session(state.database.pool())
        .await
        .map_err(|e| e.to_string())
}

/// Create a new session from a reusable configuration
#[tauri::command]
pub async fn create_session_from_reusable(
    reusable_id: String,
    session_name: Option<String>,
    state: State<'_, AudioState>,
) -> Result<AudioMixerConfiguration, String> {
    let uuid = Uuid::parse_str(&reusable_id).map_err(|e| e.to_string())?;

    AudioMixerConfiguration::create_session_from_reusable(state.database.pool(), uuid, session_name)
        .await
        .map_err(|e| e.to_string())
}

/// Save the current session configuration back to its reusable configuration
#[tauri::command]
pub async fn save_session_to_reusable(state: State<'_, AudioState>) -> Result<(), String> {
    let pool = state.database.pool();

    // Get the active session
    let mut active_session = AudioMixerConfiguration::get_active_session(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No active session found".to_string())?;

    // Get the reusable configuration it's linked to
    let reusable_id = active_session
        .reusable_configuration_id
        .ok_or_else(|| "Active session is not linked to a reusable configuration".to_string())?;

    let mut reusable_config = AudioMixerConfiguration::find_by_id(pool, reusable_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Linked reusable configuration not found".to_string())?;

    // Copy session data to reusable config (excluding session-specific fields)
    reusable_config.name = active_session.name.clone();
    reusable_config.description = active_session.description.clone();

    // Update the reusable configuration
    reusable_config
        .update(pool)
        .await
        .map_err(|e| e.to_string())?;

    // TODO: Also copy related audio devices, effects, etc.

    Ok(())
}

/// Save the current session as a new reusable configuration
#[tauri::command]
pub async fn save_session_as_new_reusable(
    name: String,
    description: Option<String>,
    state: State<'_, AudioState>,
) -> Result<AudioMixerConfiguration, String> {
    let pool = state.database.pool();

    // Get the active session
    let active_session = AudioMixerConfiguration::get_active_session(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No active session found".to_string())?;

    // Create new reusable configuration based on session
    let mut new_reusable = AudioMixerConfiguration {
        id: Uuid::new_v4(),
        name,
        description,
        configuration_type: "reusable".to_string(),
        session_active: false,
        reusable_configuration_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        deleted_at: None,
    };

    // Save the new reusable configuration
    new_reusable.save(pool).await.map_err(|e| e.to_string())?;

    // Update the active session to point to this new reusable config
    let mut updated_session = active_session;
    updated_session.reusable_configuration_id = Some(new_reusable.id);
    updated_session
        .update(pool)
        .await
        .map_err(|e| e.to_string())?;

    // TODO: Also copy related audio devices, effects, etc.

    Ok(new_reusable)
}

/// Get a configuration by ID
#[tauri::command]
pub async fn get_configuration_by_id(
    id: String,
    state: State<'_, AudioState>,
) -> Result<Option<AudioMixerConfiguration>, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;

    AudioMixerConfiguration::find_by_id(state.database.pool(), uuid)
        .await
        .map_err(|e| e.to_string())
}

/// Create a new reusable configuration
#[tauri::command]
pub async fn create_reusable_configuration(
    name: String,
    description: Option<String>,
    state: State<'_, AudioState>,
) -> Result<AudioMixerConfiguration, String> {
    let mut config = AudioMixerConfiguration {
        id: Uuid::new_v4(),
        name,
        description,
        configuration_type: "reusable".to_string(),
        session_active: false,
        reusable_configuration_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        deleted_at: None,
    };

    config
        .save(state.database.pool())
        .await
        .map_err(|e| e.to_string())?;

    Ok(config)
}

/// Get configured audio devices by configuration ID
#[tauri::command]
pub async fn get_configured_audio_devices_by_config(
    configuration_id: String,
    state: State<'_, AudioState>,
) -> Result<Vec<ConfiguredAudioDevice>, String> {
    let uuid = Uuid::parse_str(&configuration_id).map_err(|e| e.to_string())?;

    ConfiguredAudioDevice::list_for_configuration(state.database.pool(), uuid)
        .await
        .map_err(|e| e.to_string())
}
