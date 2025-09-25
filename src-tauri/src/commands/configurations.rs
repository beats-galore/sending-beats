use crate::db::seaorm_services::{AudioMixerConfigurationService, ConfiguredAudioDeviceService};
use crate::entities::{audio_mixer_configuration, configured_audio_device};
use crate::AudioState;
use anyhow::Result;
use tauri::State;
use uuid::Uuid;

/// Get all reusable configurations
#[tauri::command]
pub async fn get_reusable_configurations(
    state: State<'_, AudioState>,
) -> Result<Vec<audio_mixer_configuration::Model>, String> {
    tracing::info!("🔍 get_reusable_configurations: Starting query...");

    match AudioMixerConfigurationService::list_reusable(state.database.sea_orm()).await {
        Ok(configs) => {
            tracing::info!("✅ get_reusable_configurations: Found {} configurations", configs.len());
            for config in &configs {
                tracing::debug!("  - {}: {} ({})", config.id, config.name, config.configuration_type);
            }
            Ok(configs)
        }
        Err(e) => {
            tracing::error!("❌ get_reusable_configurations: Database error: {}", e);
            Err(e.to_string())
        }
    }
}

/// Get the currently active session configuration
/// If no active session exists, creates one from the default configuration
#[tauri::command]
pub async fn get_active_session_configuration(
    state: State<'_, AudioState>,
) -> Result<Option<audio_mixer_configuration::Model>, String> {
    tracing::info!("🔍 get_active_session_configuration: Starting query...");

    match AudioMixerConfigurationService::get_active_session(state.database.sea_orm()).await {
        Ok(session) => {
            match session {
                Some(config) => {
                    tracing::info!("✅ get_active_session_configuration: Found active session: {} ({})", config.name, config.id);
                    Ok(Some(config))
                }
                None => {
                    tracing::info!("ℹ️ get_active_session_configuration: No active session found, looking for default configuration...");

                    // Try to find a default configuration to copy
                    match AudioMixerConfigurationService::get_default_configuration(state.database.sea_orm()).await {
                        Ok(Some(default_config)) => {
                            tracing::info!("🔄 Creating active session from default configuration: {} ({})", default_config.name, default_config.id);

                            let default_uuid = uuid::Uuid::parse_str(&default_config.id)
                                .map_err(|e| format!("Invalid UUID in default config: {}", e))?;

                            match AudioMixerConfigurationService::create_session_from_reusable(
                                state.database.sea_orm(),
                                default_uuid,
                                Some(format!("{} (Active Session)", default_config.name))
                            ).await {
                                Ok(new_session) => {
                                    tracing::info!("✅ Created active session from default: {} ({})", new_session.name, new_session.id);
                                    Ok(Some(new_session))
                                }
                                Err(e) => {
                                    tracing::error!("❌ Failed to create session from default: {}", e);
                                    Err(e.to_string())
                                }
                            }
                        }
                        Ok(None) => {
                            tracing::warn!("⚠️ No default configuration found, returning None");
                            Ok(None)
                        }
                        Err(e) => {
                            tracing::error!("❌ Error looking for default configuration: {}", e);
                            Err(e.to_string())
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ get_active_session_configuration: Database error: {}", e);
            Err(e.to_string())
        }
    }
}

/// Create a new session from a reusable configuration
#[tauri::command]
pub async fn create_session_from_reusable(
    reusable_id: String,
    session_name: Option<String>,
    state: State<'_, AudioState>,
) -> Result<audio_mixer_configuration::Model, String> {
    let uuid = Uuid::parse_str(&reusable_id).map_err(|e| e.to_string())?;

    AudioMixerConfigurationService::create_session_from_reusable(state.database.sea_orm(), uuid, session_name)
        .await
        .map_err(|e| e.to_string())
}

/// Save the current session configuration back to its reusable configuration
#[tauri::command]
pub async fn save_session_to_reusable(state: State<'_, AudioState>) -> Result<(), String> {
    AudioMixerConfigurationService::save_session_to_reusable(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())
}

/// Save the current session as a new reusable configuration
#[tauri::command]
pub async fn save_session_as_new_reusable(
    name: String,
    description: Option<String>,
    state: State<'_, AudioState>,
) -> Result<audio_mixer_configuration::Model, String> {
    AudioMixerConfigurationService::save_session_as_new_reusable(state.database.sea_orm(), name, description)
        .await
        .map_err(|e| e.to_string())
}

/// Get a configuration by ID
#[tauri::command]
pub async fn get_configuration_by_id(
    id: String,
    state: State<'_, AudioState>,
) -> Result<Option<audio_mixer_configuration::Model>, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;

    AudioMixerConfigurationService::find_by_id(state.database.sea_orm(), uuid)
        .await
        .map_err(|e| e.to_string())
}

/// Create a new reusable configuration
#[tauri::command]
pub async fn create_reusable_configuration(
    name: String,
    description: Option<String>,
    state: State<'_, AudioState>,
) -> Result<audio_mixer_configuration::Model, String> {
    AudioMixerConfigurationService::create_reusable_configuration(state.database.sea_orm(), name, description)
        .await
        .map_err(|e| e.to_string())
}

/// Get configured audio devices by configuration ID
#[tauri::command]
pub async fn get_configured_audio_devices_by_config(
    configuration_id: String,
    state: State<'_, AudioState>,
) -> Result<Vec<configured_audio_device::Model>, String> {
    let uuid = Uuid::parse_str(&configuration_id).map_err(|e| e.to_string())?;

    ConfiguredAudioDeviceService::list_for_configuration(state.database.sea_orm(), uuid)
        .await
        .map_err(|e| e.to_string())
}
