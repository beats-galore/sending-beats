use crate::db::seaorm_services::{AudioMixerConfigurationService, ConfiguredAudioDeviceService};
use crate::entities::{audio_mixer_configuration, configured_audio_device};
use crate::AudioState;
use anyhow::Result;
use colored::*;
use sea_orm::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use tauri::State;
use uuid::Uuid;

/// Get all reusable configurations
#[tauri::command]
pub async fn get_reusable_configurations(
    state: State<'_, AudioState>,
) -> Result<Vec<CompleteConfigurationData>, String> {
    tracing::info!("🔍 get_reusable_configurations: Starting query...");

    match AudioMixerConfigurationService::list_reusable(state.database.sea_orm()).await {
        Ok(configs) => {
            tracing::info!(
                "✅ get_reusable_configurations: Found {} configurations",
                configs.len()
            );

            // Get complete data for each configuration
            let mut complete_configs = Vec::new();
            for config in configs {
                tracing::debug!(
                    "  - {}: {} ({})",
                    config.id,
                    config.name,
                    config.configuration_type
                );
                let complete_data = get_complete_configuration_data(&state, config).await?;
                complete_configs.push(complete_data);
            }

            Ok(complete_configs)
        }
        Err(e) => {
            tracing::error!("❌ get_reusable_configurations: Database error: {}", e);
            Err(e.to_string())
        }
    }
}

/// Get the currently active session configuration with all related data
/// Priority: active session > most recent session > create from default
#[tauri::command]
pub async fn get_active_session_configuration(
    state: State<'_, AudioState>,
) -> Result<Option<CompleteConfigurationData>, String> {
    tracing::info!("🔍 get_active_session_configuration: Starting query...");

    // First, check for an active session
    match AudioMixerConfigurationService::get_active_session(state.database.sea_orm()).await {
        Ok(Some(active_config)) => {
            tracing::info!(
                "✅ get_active_session_configuration: Found active session: {} ({})",
                active_config.name,
                active_config.id
            );

            // Get complete configuration data with all related tables (READ ONLY - no device restoration)
            let complete_data = get_complete_configuration_data(&state, active_config).await?;

            return Ok(Some(complete_data));
        }
        Ok(None) => {
            tracing::info!("ℹ️ No active session found, checking for recent sessions...");
        }
        Err(e) => {
            tracing::error!("❌ Error checking for active session: {}", e);
            return Err(e.to_string());
        }
    }

    // If no active session, check for the most recent session
    match AudioMixerConfigurationService::get_most_recent_session(state.database.sea_orm()).await {
        Ok(Some(recent_config)) => {
            tracing::info!(
                "✅ get_active_session_configuration: Found recent session: {} ({})",
                recent_config.name,
                recent_config.id
            );

            let complete_data = get_complete_configuration_data(&state, recent_config).await?;

            Ok(Some(complete_data))
        }
        Ok(None) => {
            tracing::info!("ℹ️ No sessions found, looking for default configuration to copy...");

            match AudioMixerConfigurationService::get_default_configuration(
                state.database.sea_orm(),
            )
            .await
            {
                Ok(Some(default_config)) => {
                    tracing::info!(
                        "🔄 Creating session from default configuration: {} ({})",
                        default_config.name,
                        default_config.id
                    );

                    let default_uuid = uuid::Uuid::parse_str(&default_config.id)
                        .map_err(|e| format!("Invalid UUID in default config: {}", e))?;

                    match AudioMixerConfigurationService::create_session_from_reusable(
                        state.database.sea_orm(),
                        default_uuid,
                        Some(format!("{} (Session)", default_config.name)),
                    )
                    .await
                    {
                        Ok(new_session) => {
                            tracing::info!(
                                "✅ Created session from default: {} ({})",
                                new_session.name,
                                new_session.id
                            );

                            // Get complete configuration data for the new session
                            let complete_data =
                                get_complete_configuration_data(&state, new_session).await?;

                            Ok(Some(complete_data))
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
        Err(e) => {
            tracing::error!("❌ Error checking for recent sessions: {}", e);
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

    AudioMixerConfigurationService::create_session_from_reusable(
        state.database.sea_orm(),
        uuid,
        session_name,
    )
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
    AudioMixerConfigurationService::save_session_as_new_reusable(
        state.database.sea_orm(),
        name,
        description,
    )
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
    AudioMixerConfigurationService::create_reusable_configuration(
        state.database.sea_orm(),
        name,
        description,
    )
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

// Helper functions for syncing mixer state with database

/// Create a configured_audio_device and corresponding audio_effects_default entry
pub async fn create_device_configuration(
    state: &AudioState,
    device_id: &str,
    device_name: &str,
    sample_rate: i32,
    channels: u32,
    is_input: bool,
    is_virtual: bool,
) -> Result<(), String> {
    // Get the active session configuration
    let session_config = match AudioMixerConfigurationService::get_active_session(
        state.database.sea_orm(),
    )
    .await
    {
        Ok(Some(config)) => config,
        Ok(None) => {
            // No active session, try to get most recent or create from default
            match AudioMixerConfigurationService::get_most_recent_session(state.database.sea_orm())
                .await
            {
                Ok(Some(recent_config)) => recent_config,
                Ok(None) => {
                    // Try to create session from default
                    match AudioMixerConfigurationService::get_default_configuration(
                        state.database.sea_orm(),
                    )
                    .await
                    {
                        Ok(Some(default_config)) => {
                            let default_uuid = uuid::Uuid::parse_str(&default_config.id)
                                .map_err(|e| format!("Invalid UUID in default config: {}", e))?;

                            AudioMixerConfigurationService::create_session_from_reusable(
                                state.database.sea_orm(),
                                default_uuid,
                                Some(format!("{} (Session)", default_config.name)),
                            )
                            .await
                            .map_err(|e| e.to_string())?
                        }
                        Ok(None) => {
                            tracing::warn!("No session or default configuration found, cannot create device configuration");
                            return Ok(()); // Don't fail if no session
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        Err(e) => return Err(e.to_string()),
    };

    // Check for duplicate device by device_identifier in this configuration
    tracing::info!(
        "{}: Checking for existing device with identifier: {} in config: {}",
        "DUPLICATE_CHECK".on_blue().magenta(),
        device_id,
        session_config.id
    );

    let existing_device = crate::entities::configured_audio_device::Entity::find()
        .filter(crate::entities::configured_audio_device::Column::DeviceIdentifier.eq(device_id))
        .filter(
            crate::entities::configured_audio_device::Column::ConfigurationId
                .eq(&session_config.id),
        )
        .filter(crate::entities::configured_audio_device::Column::DeletedAt.is_null())
        .one(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    if let Some(existing) = existing_device {
        tracing::info!(
            "{}: Device already exists with identifier: {} ({}), skipping creation",
            "DUPLICATE_FOUND".on_blue().magenta(),
            device_id,
            existing.id
        );
        return Ok(()); // Device already exists, skip creation
    }

    tracing::info!(
        "{}: Creating device configuration for {} ({}): device_id={}, is_input={}, channels={}",
        "CREATE_DEVICE".on_blue().magenta(),
        device_name,
        device_id,
        device_id,
        is_input,
        channels
    );

    let now = chrono::Utc::now();
    let device_uuid = uuid::Uuid::new_v4();

    // Find the next available channel number for input devices
    let next_channel_number = if is_input {
        // Get all input devices in this configuration
        let existing_input_devices = crate::entities::configured_audio_device::Entity::find()
            .filter(
                crate::entities::configured_audio_device::Column::ConfigurationId
                    .eq(&session_config.id),
            )
            .filter(crate::entities::configured_audio_device::Column::IsInput.eq(true))
            .filter(crate::entities::configured_audio_device::Column::DeletedAt.is_null())
            .all(state.database.sea_orm())
            .await
            .map_err(|e| e.to_string())?;

        // Find the highest channel number and add 1
        let max_channel = existing_input_devices
            .iter()
            .map(|d| d.channel_number)
            .max()
            .unwrap_or(-1);

        max_channel + 1
    } else {
        0 // Output devices don't use channel numbers the same way
    };

    tracing::info!(
        "{}: Assigning channel number {} to device {}",
        "CHANNEL_ASSIGN".on_blue().magenta(),
        next_channel_number,
        device_id
    );

    // Create configured_audio_device entry
    let device_entry = crate::entities::configured_audio_device::ActiveModel {
        id: sea_orm::Set(device_uuid.to_string()),
        device_identifier: sea_orm::Set(device_id.to_string()),
        device_name: sea_orm::Set(Some(device_name.to_string())),
        sample_rate: sea_orm::Set(sample_rate),
        buffer_size: sea_orm::Set(Some(8192)), // Default buffer size
        channel_format: sea_orm::Set(if channels == 1 {
            "mono".to_string()
        } else {
            "stereo".to_string()
        }),
        is_virtual: sea_orm::Set(is_virtual),
        is_input: sea_orm::Set(is_input),
        channel_number: sea_orm::Set(next_channel_number),
        configuration_id: sea_orm::Set(session_config.id.clone()),
        created_at: sea_orm::Set(now),
        updated_at: sea_orm::Set(now),
        deleted_at: sea_orm::Set(None),
    };

    // Insert device configuration
    match device_entry.insert(state.database.sea_orm()).await {
        Ok(device_model) => {
            tracing::info!(
                "✅ Created configured_audio_device: {} ({})",
                device_model.device_name.as_deref().unwrap_or("Unknown"),
                device_model.id
            );

            // Create corresponding audio_effects_default entry
            let effects_entry = crate::entities::audio_effects_default::ActiveModel {
                id: sea_orm::Set(uuid::Uuid::new_v4().to_string()),
                device_id: sea_orm::Set(device_model.id.clone()),
                configuration_id: sea_orm::Set(session_config.id.clone()),
                gain: sea_orm::Set(1.0), // Default gain (0dB)
                pan: sea_orm::Set(0.0),  // Center pan
                muted: sea_orm::Set(false),
                solo: sea_orm::Set(false),
                created_at: sea_orm::Set(now),
                updated_at: sea_orm::Set(now),
                deleted_at: sea_orm::Set(None),
            };

            match effects_entry.insert(state.database.sea_orm()).await {
                Ok(effects_model) => {
                    tracing::info!(
                        "✅ Created audio_effects_default for device: {}",
                        effects_model.device_id
                    );
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("❌ Failed to create audio_effects_default: {}", e);
                    Err(format!("Failed to create audio effects defaults: {}", e))
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to create configured_audio_device: {}", e);
            Err(format!("Failed to create device configuration: {}", e))
        }
    }
}

/// Remove a configured_audio_device and its related entries (soft delete)
pub async fn remove_device_configuration(
    state: &AudioState,
    device_id: &str,
) -> Result<(), String> {
    tracing::info!("🗑️ Removing device configuration for: {}", device_id);

    let now = chrono::Utc::now();

    // Soft delete configured_audio_device entries
    match crate::entities::configured_audio_device::Entity::update_many()
        .col_expr(
            crate::entities::configured_audio_device::Column::DeletedAt,
            Expr::val(now).into(),
        )
        .col_expr(
            crate::entities::configured_audio_device::Column::UpdatedAt,
            Expr::val(now).into(),
        )
        .filter(crate::entities::configured_audio_device::Column::DeviceIdentifier.eq(device_id))
        .filter(crate::entities::configured_audio_device::Column::DeletedAt.is_null())
        .exec(state.database.sea_orm())
        .await
    {
        Ok(result) => {
            tracing::info!(
                "✅ Soft deleted {} configured_audio_device entries",
                result.rows_affected
            );

            // For now, we'll find device IDs manually and then soft delete effects
            // This is a simpler approach that avoids complex subquery issues
            let device_configs = crate::entities::configured_audio_device::Entity::find()
                .filter(
                    crate::entities::configured_audio_device::Column::DeviceIdentifier
                        .eq(device_id),
                )
                .filter(crate::entities::configured_audio_device::Column::DeletedAt.is_null())
                .all(state.database.sea_orm())
                .await;

            match device_configs {
                Ok(configs) => {
                    for config in configs {
                        let _ = crate::entities::audio_effects_default::Entity::update_many()
                            .col_expr(
                                crate::entities::audio_effects_default::Column::DeletedAt,
                                Expr::val(now).into(),
                            )
                            .col_expr(
                                crate::entities::audio_effects_default::Column::UpdatedAt,
                                Expr::val(now).into(),
                            )
                            .filter(
                                crate::entities::audio_effects_default::Column::DeviceId
                                    .eq(&config.id),
                            )
                            .exec(state.database.sea_orm())
                            .await;
                    }
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("Failed to find device configs for effects cleanup: {}", e);
                    Ok(()) // Don't fail the entire operation
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to soft delete configured_audio_device: {}", e);
            Err(format!("Failed to remove device configuration: {}", e))
        }
    }
}

/// Information about a loaded configuration for UI synchronization
#[derive(Debug, serde::Serialize)]
pub struct LoadedConfigurationInfo {
    pub configuration_id: String,
    pub configuration_name: String,
    pub loaded_input_devices: Vec<String>, // Device IDs that were added as inputs
    pub loaded_output_devices: Vec<String>, // Device IDs that were added as outputs
    pub channels_with_devices: Vec<(u32, Option<String>)>, // (channel_id, input_device_id)
}

/// Complete configuration data including all related tables
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteConfigurationData {
    pub configuration: crate::entities::audio_mixer_configuration::Model,
    pub configured_devices: Vec<crate::entities::configured_audio_device::Model>,
    pub audio_effects_default: Vec<crate::entities::audio_effects_default::Model>,
    pub audio_effects_custom: Vec<crate::entities::audio_effects_custom::Model>,
}

/// Helper function to fetch complete configuration data with all related tables
async fn get_complete_configuration_data(
    state: &State<'_, AudioState>,
    config: crate::entities::audio_mixer_configuration::Model,
) -> Result<CompleteConfigurationData, String> {
    tracing::info!(
        "{}: Fetching data for config: {} ({})",
        "CONFIG_DATA_FETCH".on_blue().magenta(),
        config.name,
        config.id
    );
    let config_uuid = Uuid::parse_str(&config.id).map_err(|e| e.to_string())?;

    // Get configured devices
    tracing::info!(
        "{}: Querying configured devices for config ID: {}",
        "DEVICE_QUERY".on_blue().magenta(),
        config.id
    );
    let configured_devices = crate::entities::configured_audio_device::Entity::find()
        .filter(crate::entities::configured_audio_device::Column::ConfigurationId.eq(&config.id))
        .filter(crate::entities::configured_audio_device::Column::DeletedAt.is_null())
        .all(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "{}: Found {} configured devices in get_complete_configuration_data",
        "DEVICE_COUNT".on_blue().magenta(),
        configured_devices.len()
    );
    for device in &configured_devices {
        tracing::info!(
            "  {}: Device: {} ({}) - Input: {}",
            "DEVICE_DETAIL".on_blue().magenta(),
            device.device_name.as_deref().unwrap_or("Unknown"),
            device.device_identifier,
            device.is_input
        );
    }

    // Get audio effects default
    tracing::info!(
        "{}: Querying audio_effects_default for config ID: {}",
        "EFFECTS_QUERY".on_blue().magenta(),
        config.id
    );
    let audio_effects_default = crate::entities::audio_effects_default::Entity::find()
        .filter(crate::entities::audio_effects_default::Column::ConfigurationId.eq(&config.id))
        .filter(crate::entities::audio_effects_default::Column::DeletedAt.is_null())
        .all(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "{}: Found {} default effects in get_complete_configuration_data",
        "EFFECTS_DEFAULT".on_blue().magenta(),
        audio_effects_default.len()
    );

    // Get audio effects custom
    tracing::info!(
        "{}: Querying audio_effects_custom for config ID: {}",
        "EFFECTS_QUERY".on_blue().magenta(),
        config.id
    );
    let audio_effects_custom = crate::entities::audio_effects_custom::Entity::find()
        .filter(crate::entities::audio_effects_custom::Column::ConfigurationId.eq(&config.id))
        .filter(crate::entities::audio_effects_custom::Column::DeletedAt.is_null())
        .all(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "{}: Found {} custom effects in get_complete_configuration_data",
        "EFFECTS_CUSTOM".on_blue().magenta(),
        audio_effects_custom.len()
    );

    Ok(CompleteConfigurationData {
        configuration: config,
        configured_devices,
        audio_effects_default,
        audio_effects_custom,
    })
}
