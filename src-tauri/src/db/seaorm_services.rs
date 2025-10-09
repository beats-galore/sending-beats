use crate::entities::{
    audio_effects_custom, audio_effects_default, audio_mixer_configuration,
    configured_audio_device, system_audio_state,
};
use anyhow::Result;
use sea_orm::*;
use sea_orm::{prelude::Expr, Set};
use uuid::Uuid;

/// SeaORM-based service for audio mixer configurations
pub struct AudioMixerConfigurationService;

impl AudioMixerConfigurationService {
    /// Get all reusable configurations
    pub async fn list_reusable(
        db: &DatabaseConnection,
    ) -> Result<Vec<audio_mixer_configuration::Model>> {
        let configs = audio_mixer_configuration::Entity::find()
            .filter(audio_mixer_configuration::Column::ConfigurationType.eq("reusable"))
            .order_by_desc(audio_mixer_configuration::Column::CreatedAt)
            .all(db)
            .await?;

        Ok(configs)
    }

    /// Get the currently active session configuration
    pub async fn get_active_session(
        db: &DatabaseConnection,
    ) -> Result<Option<audio_mixer_configuration::Model>> {
        let config = audio_mixer_configuration::Entity::find()
            .filter(audio_mixer_configuration::Column::SessionActive.eq(true))
            .one(db)
            .await?;

        Ok(config)
    }

    /// Find configuration by ID
    pub async fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<audio_mixer_configuration::Model>> {
        let config = audio_mixer_configuration::Entity::find_by_id(id.to_string())
            .one(db)
            .await?;

        Ok(config)
    }

    /// Get the default configuration (marked with is_default = true)
    pub async fn get_default_configuration(
        db: &DatabaseConnection,
    ) -> Result<Option<audio_mixer_configuration::Model>> {
        let config = audio_mixer_configuration::Entity::find()
            .filter(audio_mixer_configuration::Column::IsDefault.eq(true))
            .one(db)
            .await?;

        Ok(config)
    }

    /// Get the most recent session configuration (regardless of active status)
    pub async fn get_most_recent_session(
        db: &DatabaseConnection,
    ) -> Result<Option<audio_mixer_configuration::Model>> {
        let config = audio_mixer_configuration::Entity::find()
            .filter(audio_mixer_configuration::Column::ConfigurationType.eq("session"))
            .order_by_desc(audio_mixer_configuration::Column::UpdatedAt)
            .one(db)
            .await?;

        Ok(config)
    }

    /// Set configuration as active session (deactivates all others)
    pub async fn set_as_active_session(db: &DatabaseConnection, config_id: Uuid) -> Result<()> {
        // Start transaction
        let txn = db.begin().await?;

        // Deactivate all other sessions
        audio_mixer_configuration::Entity::update_many()
            .col_expr(
                audio_mixer_configuration::Column::SessionActive,
                Expr::value(false),
            )
            .col_expr(
                audio_mixer_configuration::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(audio_mixer_configuration::Column::SessionActive.eq(true))
            .exec(&txn)
            .await?;

        // Activate the specified configuration
        audio_mixer_configuration::Entity::update_many()
            .col_expr(
                audio_mixer_configuration::Column::SessionActive,
                Expr::value(true),
            )
            .col_expr(
                audio_mixer_configuration::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(audio_mixer_configuration::Column::Id.eq(config_id.to_string()))
            .exec(&txn)
            .await?;

        txn.commit().await?;
        Ok(())
    }

    /// Create a new session from a reusable configuration
    /// Copies all related audio devices and effects
    pub async fn create_session_from_reusable(
        db: &DatabaseConnection,
        reusable_id: Uuid,
        session_name: Option<String>,
    ) -> Result<audio_mixer_configuration::Model> {
        tracing::info!(
            "🔄 Creating session from reusable configuration: {}",
            reusable_id
        );

        // Get the reusable configuration
        let reusable = Self::find_by_id(db, reusable_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Reusable configuration not found"))?;

        let session_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        // Start transaction
        let txn = db.begin().await?;

        // Create new session configuration
        let session_active_model = audio_mixer_configuration::ActiveModel {
            id: Set(session_id.to_string()),
            name: Set(session_name.unwrap_or_else(|| format!("{} (Session)", reusable.name))),
            description: Set(reusable.description.clone()),
            configuration_type: Set("session".to_string()),
            session_active: Set(true),
            reusable_configuration_id: Set(Some(reusable_id.to_string())),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        // Deactivate all other sessions first
        audio_mixer_configuration::Entity::update_many()
            .col_expr(
                audio_mixer_configuration::Column::SessionActive,
                Expr::value(false),
            )
            .col_expr(
                audio_mixer_configuration::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(audio_mixer_configuration::Column::SessionActive.eq(true))
            .exec(&txn)
            .await?;

        // Insert new session
        let session_model = session_active_model.insert(&txn).await?;

        tracing::info!(
            "✅ Created session configuration: {} ({})",
            session_model.name,
            session_model.id
        );

        // Copy related configured audio devices
        let audio_devices = configured_audio_device::Entity::find()
            .filter(configured_audio_device::Column::ConfigurationId.eq(reusable_id.to_string()))
            .all(&txn)
            .await?;

        for original_device in audio_devices {
            let new_device = configured_audio_device::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_identifier: Set(original_device.device_identifier),
                device_name: Set(original_device.device_name),
                sample_rate: Set(original_device.sample_rate),
                buffer_size: Set(original_device.buffer_size),
                channel_format: Set(original_device.channel_format),
                is_virtual: Set(original_device.is_virtual),
                is_input: Set(original_device.is_input),
                channel_number: Set(original_device.channel_number), // Copy channel assignment
                configuration_id: Set(session_id.to_string()),       // Link to new session
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_device.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio device: {} -> new device",
                original_device.id
            );
        }

        // Copy related AudioEffectsDefault settings
        let audio_defaults = audio_effects_default::Entity::find()
            .filter(audio_effects_default::Column::ConfigurationId.eq(reusable_id.to_string()))
            .all(&txn)
            .await?;

        for original_default in audio_defaults {
            let new_default = audio_effects_default::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_id: Set(original_default.device_id),
                configuration_id: Set(session_id.to_string()), // Link to new session
                gain: Set(original_default.gain),
                pan: Set(original_default.pan),
                muted: Set(original_default.muted),
                solo: Set(original_default.solo),
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_default.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio default: {} -> new default",
                original_default.id
            );
        }

        // Copy related AudioEffectsCustom
        let audio_effects = audio_effects_custom::Entity::find()
            .filter(audio_effects_custom::Column::ConfigurationId.eq(reusable_id.to_string()))
            .all(&txn)
            .await?;

        for original_effect in audio_effects {
            let new_effect = audio_effects_custom::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_id: Set(original_effect.device_id),
                configuration_id: Set(session_id.to_string()), // Link to new session
                effect_type: Set(original_effect.effect_type),
                parameters: Set(original_effect.parameters),
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_effect.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio effect: {} -> new effect",
                original_effect.id
            );
        }

        txn.commit().await?;
        tracing::info!("✅ Session creation completed with all related data copied");

        Ok(session_model)
    }

    /// Save current session back to its linked reusable configuration
    pub async fn save_session_to_reusable(db: &DatabaseConnection) -> Result<()> {
        // Get the active session
        let active_session = Self::get_active_session(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No active session found"))?;

        // Get the reusable configuration it's linked to
        let reusable_id = active_session.reusable_configuration_id.ok_or_else(|| {
            anyhow::anyhow!("Active session is not linked to a reusable configuration")
        })?;

        let reusable_uuid = Uuid::parse_str(&reusable_id)
            .map_err(|e| anyhow::anyhow!("Invalid UUID format: {}", e))?;
        let reusable_config = Self::find_by_id(db, reusable_uuid)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Linked reusable configuration not found"))?;

        // Update the reusable configuration
        let mut reusable_active: audio_mixer_configuration::ActiveModel = reusable_config.into();
        reusable_active.name = Set(active_session.name.clone());
        reusable_active.description = Set(active_session.description.clone());
        reusable_active.updated_at = Set(chrono::Utc::now());

        reusable_active.update(db).await?;

        tracing::info!(
            "✅ Updated reusable configuration basic info: {} ({})",
            active_session.name,
            reusable_id
        );

        // Start transaction to replace all related models
        let txn = db.begin().await?;

        // 1. First, delete all existing related models from the reusable configuration
        let now = chrono::Utc::now();

        // Delete existing configured_audio_devices
        configured_audio_device::Entity::delete_many()
            .filter(configured_audio_device::Column::ConfigurationId.eq(&reusable_id))
            .exec(&txn)
            .await?;

        // Delete existing audio_effects_default
        audio_effects_default::Entity::delete_many()
            .filter(audio_effects_default::Column::ConfigurationId.eq(&reusable_id))
            .exec(&txn)
            .await?;

        // Delete existing audio_effects_custom
        audio_effects_custom::Entity::delete_many()
            .filter(audio_effects_custom::Column::ConfigurationId.eq(&reusable_id))
            .exec(&txn)
            .await?;

        tracing::info!("✅ Deleted existing related models from reusable configuration");

        // 2. Copy all related models from the active session to the reusable configuration

        // Copy configured_audio_devices from session to reusable
        let session_devices = configured_audio_device::Entity::find()
            .filter(configured_audio_device::Column::ConfigurationId.eq(&active_session.id))
            .all(&txn)
            .await?;

        for original_device in session_devices {
            let new_device = configured_audio_device::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_identifier: Set(original_device.device_identifier),
                device_name: Set(original_device.device_name),
                sample_rate: Set(original_device.sample_rate),
                buffer_size: Set(original_device.buffer_size),
                channel_format: Set(original_device.channel_format),
                is_virtual: Set(original_device.is_virtual),
                is_input: Set(original_device.is_input),
                channel_number: Set(original_device.channel_number), // Copy channel assignment
                configuration_id: Set(reusable_id.clone()),          // Link to reusable config
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_device.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio device: {} -> reusable config",
                original_device.id
            );
        }

        // Copy audio_effects_default from session to reusable
        let session_defaults = audio_effects_default::Entity::find()
            .filter(audio_effects_default::Column::ConfigurationId.eq(&active_session.id))
            .all(&txn)
            .await?;

        for original_default in session_defaults {
            let new_default = audio_effects_default::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_id: Set(original_default.device_id),
                configuration_id: Set(reusable_id.clone()), // Link to reusable config
                gain: Set(original_default.gain),
                pan: Set(original_default.pan),
                muted: Set(original_default.muted),
                solo: Set(original_default.solo),
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_default.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio default: {} -> reusable config",
                original_default.id
            );
        }

        // Copy audio_effects_custom from session to reusable
        let session_effects = audio_effects_custom::Entity::find()
            .filter(audio_effects_custom::Column::ConfigurationId.eq(&active_session.id))
            .all(&txn)
            .await?;

        for original_effect in session_effects {
            let new_effect = audio_effects_custom::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_id: Set(original_effect.device_id),
                configuration_id: Set(reusable_id.clone()), // Link to reusable config
                effect_type: Set(original_effect.effect_type),
                parameters: Set(original_effect.parameters),
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_effect.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio effect: {} -> reusable config",
                original_effect.id
            );
        }

        txn.commit().await?;
        tracing::info!(
            "✅ Successfully saved session back to reusable configuration with all related data"
        );

        Ok(())
    }

    /// Save current session as a new reusable configuration
    pub async fn save_session_as_new_reusable(
        db: &DatabaseConnection,
        name: String,
        description: Option<String>,
    ) -> Result<audio_mixer_configuration::Model> {
        // Get the active session
        let active_session = Self::get_active_session(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No active session found"))?;

        let new_reusable_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        // Start transaction
        let txn = db.begin().await?;

        // Create new reusable configuration based on session
        let new_reusable = audio_mixer_configuration::ActiveModel {
            id: Set(new_reusable_id.to_string()),
            name: Set(name),
            description: Set(description),
            configuration_type: Set("reusable".to_string()),
            session_active: Set(false),
            reusable_configuration_id: Set(None),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let new_reusable_model = new_reusable.insert(&txn).await?;

        // Store session ID before moving active_session
        let active_session_id = active_session.id.clone();

        // Update the active session to point to this new reusable config
        let mut updated_session: audio_mixer_configuration::ActiveModel = active_session.into();
        updated_session.reusable_configuration_id = Set(Some(new_reusable_id.to_string()));
        updated_session.updated_at = Set(now);
        updated_session.update(&txn).await?;

        tracing::info!(
            "✅ Created new reusable configuration: {} ({})",
            new_reusable_model.name,
            new_reusable_model.id
        );

        // Copy all related models from the active session to the new reusable configuration

        // Copy configured_audio_devices from session to new reusable
        let session_devices = configured_audio_device::Entity::find()
            .filter(configured_audio_device::Column::ConfigurationId.eq(&active_session_id))
            .all(&txn)
            .await?;

        for original_device in session_devices {
            let new_device = configured_audio_device::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_identifier: Set(original_device.device_identifier),
                device_name: Set(original_device.device_name),
                sample_rate: Set(original_device.sample_rate),
                buffer_size: Set(original_device.buffer_size),
                channel_format: Set(original_device.channel_format),
                is_virtual: Set(original_device.is_virtual),
                is_input: Set(original_device.is_input),
                channel_number: Set(original_device.channel_number), // Copy channel assignment
                configuration_id: Set(new_reusable_id.to_string()),  // Link to new reusable config
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_device.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio device: {} -> new reusable config",
                original_device.id
            );
        }

        // Copy audio_effects_default from session to new reusable
        let session_defaults = audio_effects_default::Entity::find()
            .filter(audio_effects_default::Column::ConfigurationId.eq(&active_session_id))
            .all(&txn)
            .await?;

        for original_default in session_defaults {
            let new_default = audio_effects_default::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_id: Set(original_default.device_id),
                configuration_id: Set(new_reusable_id.to_string()), // Link to new reusable config
                gain: Set(original_default.gain),
                pan: Set(original_default.pan),
                muted: Set(original_default.muted),
                solo: Set(original_default.solo),
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_default.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio default: {} -> new reusable config",
                original_default.id
            );
        }

        // Copy audio_effects_custom from session to new reusable
        let session_effects = audio_effects_custom::Entity::find()
            .filter(audio_effects_custom::Column::ConfigurationId.eq(&active_session_id))
            .all(&txn)
            .await?;

        for original_effect in session_effects {
            let new_effect = audio_effects_custom::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                device_id: Set(original_effect.device_id),
                configuration_id: Set(new_reusable_id.to_string()), // Link to new reusable config
                effect_type: Set(original_effect.effect_type),
                parameters: Set(original_effect.parameters),
                created_at: Set(now),
                updated_at: Set(now),
            };

            new_effect.insert(&txn).await?;
            tracing::debug!(
                "✅ Copied audio effect: {} -> new reusable config",
                original_effect.id
            );
        }

        txn.commit().await?;
        tracing::info!(
            "✅ Successfully saved session as new reusable configuration with all related data"
        );

        Ok(new_reusable_model)
    }

    /// Create a new reusable configuration
    pub async fn create_reusable_configuration(
        db: &DatabaseConnection,
        name: String,
        description: Option<String>,
    ) -> Result<audio_mixer_configuration::Model> {
        let new_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let new_config = audio_mixer_configuration::ActiveModel {
            id: Set(new_id.to_string()),
            name: Set(name),
            description: Set(description),
            configuration_type: Set("reusable".to_string()),
            session_active: Set(false),
            reusable_configuration_id: Set(None),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = new_config.insert(db).await?;
        Ok(model)
    }
}

/// SeaORM-based service for configured audio devices
pub struct ConfiguredAudioDeviceService;

impl ConfiguredAudioDeviceService {
    /// Get a device by its ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: &str,
    ) -> Result<Option<configured_audio_device::Model>> {
        let device = configured_audio_device::Entity::find_by_id(id)
            .one(db)
            .await?;

        Ok(device)
    }

    /// List devices for a configuration
    pub async fn list_for_configuration(
        db: &DatabaseConnection,
        configuration_id: Uuid,
    ) -> Result<Vec<configured_audio_device::Model>> {
        let devices = configured_audio_device::Entity::find()
            .filter(configured_audio_device::Column::ConfigurationId.eq(configuration_id))
            .order_by_desc(configured_audio_device::Column::CreatedAt)
            .all(db)
            .await?;

        Ok(devices)
    }

    /// Get channel number for a device identifier from the active configuration
    pub async fn get_channel_number_for_active_device(
        db: &DatabaseConnection,
        device_identifier: &str,
    ) -> Result<Option<u32>> {
        // First, get the active session configuration
        let active_config = audio_mixer_configuration::Entity::find()
            .filter(audio_mixer_configuration::Column::SessionActive.eq(true))
            .one(db)
            .await?;

        let active_config = match active_config {
            Some(config) => config,
            None => return Ok(None), // No active configuration
        };

        // Find the device in the active configuration
        let device = configured_audio_device::Entity::find()
            .filter(configured_audio_device::Column::ConfigurationId.eq(&active_config.id))
            .filter(configured_audio_device::Column::DeviceIdentifier.eq(device_identifier))
            .one(db)
            .await?;

        match device {
            Some(device) => Ok(Some(device.channel_number as u32)),
            None => Ok(None),
        }
    }
}

pub struct AudioEffectsDefaultService;

impl AudioEffectsDefaultService {
    pub async fn find_by_device_id(
        db: &DatabaseConnection,
        device_id: &str,
    ) -> Result<Option<audio_effects_default::Model>> {
        let effect = audio_effects_default::Entity::find()
            .filter(audio_effects_default::Column::DeviceId.eq(device_id))
            .one(db)
            .await?;

        Ok(effect)
    }

    /// Find effects by device identifier string (not UUID) in the active configuration
    pub async fn find_by_device_identifier_in_active_config(
        db: &DatabaseConnection,
        device_identifier: &str,
    ) -> Result<Option<audio_effects_default::Model>> {
        // First, get the active session configuration
        let active_config = audio_mixer_configuration::Entity::find()
            .filter(audio_mixer_configuration::Column::SessionActive.eq(true))
            .one(db)
            .await?;

        let active_config = match active_config {
            Some(config) => config,
            None => return Ok(None),
        };

        // Find the configured_audio_device by device_identifier in the active config
        let device = configured_audio_device::Entity::find()
            .filter(configured_audio_device::Column::ConfigurationId.eq(&active_config.id))
            .filter(configured_audio_device::Column::DeviceIdentifier.eq(device_identifier))
            .one(db)
            .await?;

        let device = match device {
            Some(d) => d,
            None => return Ok(None),
        };

        // Now find the effects for this device
        let effect = audio_effects_default::Entity::find()
            .filter(audio_effects_default::Column::DeviceId.eq(&device.id))
            .filter(audio_effects_default::Column::ConfigurationId.eq(&active_config.id))
            .one(db)
            .await?;

        Ok(effect)
    }

    pub async fn list_for_configuration(
        db: &DatabaseConnection,
        configuration_id: &str,
    ) -> Result<Vec<audio_effects_default::Model>> {
        let effects = audio_effects_default::Entity::find()
            .filter(audio_effects_default::Column::ConfigurationId.eq(configuration_id))
            .all(db)
            .await?;

        Ok(effects)
    }

    pub async fn update_gain(
        db: &DatabaseConnection,
        effects_id: &str,
        gain: f32,
    ) -> Result<audio_effects_default::Model> {
        let effect = audio_effects_default::Entity::find_by_id(effects_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Audio effects default not found"))?;

        let mut active: audio_effects_default::ActiveModel = effect.into();
        active.gain = Set(gain);
        active.updated_at = Set(chrono::Utc::now());

        let updated = active.update(db).await?;
        Ok(updated)
    }

    pub async fn update_pan(
        db: &DatabaseConnection,
        effects_id: &str,
        pan: f32,
    ) -> Result<audio_effects_default::Model> {
        let effect = audio_effects_default::Entity::find_by_id(effects_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Audio effects default not found"))?;

        let mut active: audio_effects_default::ActiveModel = effect.into();
        active.pan = Set(pan);
        active.updated_at = Set(chrono::Utc::now());

        let updated = active.update(db).await?;
        Ok(updated)
    }

    pub async fn update_mute(
        db: &DatabaseConnection,
        effects_id: &str,
        muted: bool,
    ) -> Result<audio_effects_default::Model> {
        let effect = audio_effects_default::Entity::find_by_id(effects_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Audio effects default not found"))?;

        let mut active: audio_effects_default::ActiveModel = effect.into();
        active.muted = Set(muted);
        active.updated_at = Set(chrono::Utc::now());

        let updated = active.update(db).await?;
        Ok(updated)
    }

    pub async fn update_solo(
        db: &DatabaseConnection,
        configuration_id: &str,
        effects_id: &str,
        solo: bool,
    ) -> Result<audio_effects_default::Model> {
        let txn = db.begin().await?;

        if solo {
            audio_effects_default::Entity::update_many()
                .col_expr(audio_effects_default::Column::Solo, Expr::value(false))
                .col_expr(
                    audio_effects_default::Column::UpdatedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(audio_effects_default::Column::ConfigurationId.eq(configuration_id))
                .exec(&txn)
                .await?;
        }

        let effect = audio_effects_default::Entity::find_by_id(effects_id)
            .one(&txn)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Audio effects default not found"))?;

        let mut active: audio_effects_default::ActiveModel = effect.into();
        active.solo = Set(solo);
        active.updated_at = Set(chrono::Utc::now());

        let updated = active.update(&txn).await?;

        txn.commit().await?;
        Ok(updated)
    }
}

pub struct SystemAudioStateService;

impl SystemAudioStateService {
    /// Get or create the system audio state (singleton pattern)
    pub async fn get_or_create(db: &DatabaseConnection) -> Result<system_audio_state::Model> {
        let state = system_audio_state::Entity::find().one(db).await?;

        match state {
            Some(state) => Ok(state),
            None => {
                let new_state = system_audio_state::Model::new();
                let created = new_state.insert(db).await?;
                Ok(created)
            }
        }
    }

    /// Update dummy aggregate device UID
    pub async fn set_dummy_device_uid(
        db: &DatabaseConnection,
        device_uid: Option<String>,
    ) -> Result<system_audio_state::Model> {
        let state = Self::get_or_create(db).await?;

        let mut active: system_audio_state::ActiveModel = state.into();
        active.dummy_aggregate_device_uid = Set(device_uid);
        active.updated_at = Set(chrono::Utc::now());

        let updated = active.update(db).await?;
        Ok(updated)
    }

    /// Set diversion state (whether system audio is diverted to dummy device)
    pub async fn set_diversion_state(
        db: &DatabaseConnection,
        is_diverted: bool,
        previous_default_uid: Option<String>,
    ) -> Result<system_audio_state::Model> {
        let state = Self::get_or_create(db).await?;

        let mut active: system_audio_state::ActiveModel = state.into();
        active.is_diverted = Set(is_diverted);
        active.previous_default_device_uid = Set(previous_default_uid);
        active.updated_at = Set(chrono::Utc::now());

        let updated = active.update(db).await?;
        Ok(updated)
    }

    /// Reset to non-diverted state (clear previous default)
    pub async fn reset_diversion(db: &DatabaseConnection) -> Result<system_audio_state::Model> {
        Self::set_diversion_state(db, false, None).await
    }
}
