use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Configured audio device
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConfiguredAudioDevice {
    pub id: Uuid,
    pub device_identifier: String,
    pub device_name: Option<String>,
    pub sample_rate: i32,
    pub buffer_size: Option<i32>,
    pub channel_format: String, // 'stereo' or 'mono'
    pub is_virtual: bool,
    pub is_input: bool,
    pub configuration_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Simplified audio device config for backwards compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceConfig {
    pub id: String,
    pub name: String,
    pub device_type: String, // "input" or "output"
    pub sample_rate: u32,
    pub channels: u32,
    pub is_default: bool,
    pub is_active: bool,
    pub last_seen: i64,
}

impl ConfiguredAudioDevice {
    /// Create new configured audio device
    pub fn new(
        device_identifier: String,
        configuration_id: Uuid,
        sample_rate: i32,
        channel_format: String,
        is_input: bool,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            device_identifier,
            device_name: None,
            sample_rate,
            buffer_size: None,
            channel_format,
            is_virtual: false,
            is_input,
            configuration_id,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Save device to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO configured_audio_devices
             (id, device_identifier, device_name, sample_rate, buffer_size, channel_format,
              is_virtual, is_input, configuration_id, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(self.id.to_string())
        .bind(&self.device_identifier)
        .bind(&self.device_name)
        .bind(self.sample_rate)
        .bind(self.buffer_size)
        .bind(&self.channel_format)
        .bind(self.is_virtual)
        .bind(self.is_input)
        .bind(self.configuration_id.to_string())
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update device in database
    pub async fn update(&mut self, pool: &SqlitePool) -> Result<()> {
        self.updated_at = chrono::Utc::now();

        sqlx::query(
            "UPDATE configured_audio_devices
             SET device_identifier = ?, device_name = ?, sample_rate = ?, buffer_size = ?,
                 channel_format = ?, is_virtual = ?, is_input = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&self.device_identifier)
        .bind(&self.device_name)
        .bind(self.sample_rate)
        .bind(self.buffer_size)
        .bind(&self.channel_format)
        .bind(self.is_virtual)
        .bind(self.is_input)
        .bind(self.updated_at)
        .bind(self.id.to_string())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Soft delete device
    pub async fn delete(&mut self, pool: &SqlitePool) -> Result<()> {
        self.deleted_at = Some(chrono::Utc::now());

        sqlx::query(
            "UPDATE configured_audio_devices
             SET deleted_at = ?
             WHERE id = ?",
        )
        .bind(self.deleted_at)
        .bind(self.id.to_string())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Find device by ID
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let device = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", device_identifier, device_name, sample_rate, buffer_size,
             channel_format, is_virtual, is_input, configuration_id as \"configuration_id: Uuid\",
             created_at, updated_at, deleted_at
             FROM configured_audio_devices
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;

        Ok(device)
    }

    /// List devices for a configuration
    pub async fn list_for_configuration(
        pool: &SqlitePool,
        configuration_id: Uuid,
    ) -> Result<Vec<Self>> {
        let devices = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", device_identifier, device_name, sample_rate, buffer_size,
             channel_format, is_virtual, is_input, configuration_id as \"configuration_id: Uuid\",
             created_at, updated_at, deleted_at
             FROM configured_audio_devices
             WHERE configuration_id = ? AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(configuration_id.to_string())
        .fetch_all(pool)
        .await?;

        Ok(devices)
    }

    /// List input devices for a configuration
    pub async fn list_inputs_for_configuration(
        pool: &SqlitePool,
        configuration_id: Uuid,
    ) -> Result<Vec<Self>> {
        let devices = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", device_identifier, device_name, sample_rate, buffer_size,
             channel_format, is_virtual, is_input, configuration_id as \"configuration_id: Uuid\",
             created_at, updated_at, deleted_at
             FROM configured_audio_devices
             WHERE configuration_id = ? AND is_input = TRUE AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(configuration_id.to_string())
        .fetch_all(pool)
        .await?;

        Ok(devices)
    }

    /// List output devices for a configuration
    pub async fn list_outputs_for_configuration(
        pool: &SqlitePool,
        configuration_id: Uuid,
    ) -> Result<Vec<Self>> {
        let devices = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", device_identifier, device_name, sample_rate, buffer_size,
             channel_format, is_virtual, is_input, configuration_id as \"configuration_id: Uuid\",
             created_at, updated_at, deleted_at
             FROM configured_audio_devices
             WHERE configuration_id = ? AND is_input = FALSE AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(configuration_id.to_string())
        .fetch_all(pool)
        .await?;

        Ok(devices)
    }

    /// Convert to simplified format for backwards compatibility
    pub fn to_simplified(&self) -> AudioDeviceConfig {
        AudioDeviceConfig {
            id: self.id.to_string(),
            name: self
                .device_name
                .clone()
                .unwrap_or_else(|| self.device_identifier.clone()),
            device_type: if self.is_input { "input" } else { "output" }.to_string(),
            sample_rate: self.sample_rate as u32,
            channels: match self.channel_format.as_str() {
                "stereo" => 2,
                "mono" => 1,
                _ => 2, // Default to stereo
            },
            is_default: false, // This field is deprecated in new schema
            is_active: true,   // This field is deprecated in new schema
            last_seen: self.updated_at.timestamp(),
        }
    }
}
