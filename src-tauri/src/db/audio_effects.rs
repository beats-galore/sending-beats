use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use uuid::Uuid;

/// Default audio effects - basic channel controls
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AudioEffectsDefault {
    pub id: Uuid,
    pub device_id: Uuid,
    pub configuration_id: Uuid,
    pub gain: f64,
    pub pan: f64,
    pub muted: bool,
    pub solo: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Custom audio effects - complex effects with JSON parameters
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AudioEffectsCustom {
    pub id: Uuid,
    pub device_id: Uuid,
    pub configuration_id: Uuid,
    pub effect_type: String, // 'equalizer', 'compressor', 'limiter', etc.
    pub parameters: String,  // JSON string - we'll convert to/from HashMap
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Channel configuration with all mixer settings (backwards compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub id: Option<u32>, // None for new channels
    pub name: String,
    pub input_device_id: Option<String>,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub effects_enabled: bool,

    // EQ settings
    pub eq_low_gain: f32,
    pub eq_mid_gain: f32,
    pub eq_high_gain: f32,

    // Compressor settings
    pub comp_enabled: bool,
    pub comp_threshold: f32,
    pub comp_ratio: f32,
    pub comp_attack: f32,
    pub comp_release: f32,

    // Limiter settings
    pub limiter_enabled: bool,
    pub limiter_threshold: f32,
}

impl AudioEffectsDefault {
    /// Create new default effects
    pub fn new(device_id: Uuid, configuration_id: Uuid) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            device_id,
            configuration_id,
            gain: 0.0,
            pan: 0.0,
            muted: false,
            solo: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Save to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO audio_effects_default
             (id, device_id, configuration_id, gain, pan, muted, solo, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(self.id.to_string())
        .bind(self.device_id.to_string())
        .bind(self.configuration_id.to_string())
        .bind(self.gain)
        .bind(self.pan)
        .bind(self.muted)
        .bind(self.solo)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update in database
    pub async fn update(&mut self, pool: &SqlitePool) -> Result<()> {
        self.updated_at = chrono::Utc::now();

        sqlx::query(
            "UPDATE audio_effects_default
             SET gain = ?, pan = ?, muted = ?, solo = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(self.gain)
        .bind(self.pan)
        .bind(self.muted)
        .bind(self.solo)
        .bind(self.updated_at)
        .bind(self.id.to_string())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Find for device
    pub async fn find_for_device(pool: &SqlitePool, device_id: Uuid) -> Result<Option<Self>> {
        let effects = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", device_id as \"device_id: Uuid\",
             configuration_id as \"configuration_id: Uuid\", gain, pan, muted, solo,
             created_at, updated_at, deleted_at
             FROM audio_effects_default
             WHERE device_id = ? AND deleted_at IS NULL",
        )
        .bind(device_id.to_string())
        .fetch_optional(pool)
        .await?;

        Ok(effects)
    }
}

impl AudioEffectsCustom {
    /// Create new custom effects
    pub fn new(
        device_id: Uuid,
        configuration_id: Uuid,
        effect_type: String,
        parameters: HashMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let now = chrono::Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            device_id,
            configuration_id,
            effect_type,
            parameters: serde_json::to_string(&parameters)?,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
    }

    /// Get parameters as HashMap
    pub fn get_parameters(&self) -> Result<HashMap<String, serde_json::Value>> {
        Ok(serde_json::from_str(&self.parameters)?)
    }

    /// Set parameters from HashMap
    pub fn set_parameters(&mut self, parameters: HashMap<String, serde_json::Value>) -> Result<()> {
        self.parameters = serde_json::to_string(&parameters)?;
        Ok(())
    }

    /// Save to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO audio_effects_custom
             (id, device_id, configuration_id, type, parameters, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(self.id.to_string())
        .bind(self.device_id.to_string())
        .bind(self.configuration_id.to_string())
        .bind(&self.effect_type)
        .bind(&self.parameters)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update in database
    pub async fn update(&mut self, pool: &SqlitePool) -> Result<()> {
        self.updated_at = chrono::Utc::now();

        sqlx::query(
            "UPDATE audio_effects_custom
             SET type = ?, parameters = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&self.effect_type)
        .bind(&self.parameters)
        .bind(self.updated_at)
        .bind(self.id.to_string())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// List custom effects for device
    pub async fn list_for_device(pool: &SqlitePool, device_id: Uuid) -> Result<Vec<Self>> {
        let effects = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", device_id as \"device_id: Uuid\",
             configuration_id as \"configuration_id: Uuid\", type as effect_type, parameters,
             created_at, updated_at, deleted_at
             FROM audio_effects_custom
             WHERE device_id = ? AND deleted_at IS NULL
             ORDER BY created_at ASC",
        )
        .bind(device_id.to_string())
        .fetch_all(pool)
        .await?;

        Ok(effects)
    }

    /// List custom effects by type for device
    pub async fn list_for_device_by_type(
        pool: &SqlitePool,
        device_id: Uuid,
        effect_type: &str,
    ) -> Result<Vec<Self>> {
        let effects = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", device_id as \"device_id: Uuid\",
             configuration_id as \"configuration_id: Uuid\", type as effect_type, parameters,
             created_at, updated_at, deleted_at
             FROM audio_effects_custom
             WHERE device_id = ? AND type = ? AND deleted_at IS NULL
             ORDER BY created_at ASC",
        )
        .bind(device_id.to_string())
        .bind(effect_type)
        .fetch_all(pool)
        .await?;

        Ok(effects)
    }
}
