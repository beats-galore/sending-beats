use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// VU meter level data for real-time buffering
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VULevelData {
    pub id: Uuid,
    pub audio_device_id: Uuid,
    pub configuration_id: Uuid,
    pub peak_left: f32,
    pub peak_right: f32,
    pub rms_left: f32,
    pub rms_right: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Simplified VU level data for real-time processing (matches old interface)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplifiedVULevelData {
    pub timestamp: i64, // Microseconds since Unix epoch
    pub channel_id: u32,
    pub peak_left: f32,
    pub rms_left: f32,
    pub peak_right: Option<f32>, // None for mono sources
    pub rms_right: Option<f32>,  // None for mono sources
    pub is_stereo: bool,
}

impl VULevelData {
    /// Create new VU level data
    pub fn new(
        audio_device_id: Uuid,
        configuration_id: Uuid,
        peak_left: f32,
        peak_right: f32,
        rms_left: f32,
        rms_right: f32,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            audio_device_id,
            configuration_id,
            peak_left,
            peak_right,
            rms_left,
            rms_right,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Insert VU level data (optimized for high-frequency inserts)
    pub async fn insert_batch(pool: &SqlitePool, levels: &[VULevelData]) -> Result<()> {
        if levels.is_empty() {
            return Ok(());
        }

        let mut tx = pool.begin().await?;

        for level in levels {
            sqlx::query(
                "INSERT INTO audio_device_levels
                 (id, audio_device_id, configuration_id, peak_left, peak_right, rms_left, rms_right, created_at, updated_at, deleted_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(level.id.to_string())
            .bind(level.audio_device_id.to_string())
            .bind(level.configuration_id.to_string())
            .bind(level.peak_left)
            .bind(level.peak_right)
            .bind(level.rms_left)
            .bind(level.rms_right)
            .bind(level.created_at)
            .bind(level.updated_at)
            .bind(level.deleted_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get recent VU levels for an audio device
    pub async fn get_recent_for_device(
        pool: &SqlitePool,
        audio_device_id: Uuid,
        limit: i64,
    ) -> Result<Vec<VULevelData>> {
        let levels = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", audio_device_id as \"audio_device_id: Uuid\",
             configuration_id as \"configuration_id: Uuid\", peak_left, peak_right, rms_left, rms_right,
             created_at, updated_at, deleted_at
             FROM audio_device_levels
             WHERE audio_device_id = ? AND deleted_at IS NULL
             ORDER BY created_at DESC
             LIMIT ?"
        )
        .bind(audio_device_id.to_string())
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(levels)
    }

    /// Get recent VU levels for a configuration
    pub async fn get_recent_for_configuration(
        pool: &SqlitePool,
        configuration_id: Uuid,
        limit: i64,
    ) -> Result<Vec<VULevelData>> {
        let levels = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", audio_device_id as \"audio_device_id: Uuid\",
             configuration_id as \"configuration_id: Uuid\", peak_left, peak_right, rms_left, rms_right,
             created_at, updated_at, deleted_at
             FROM audio_device_levels
             WHERE configuration_id = ? AND deleted_at IS NULL
             ORDER BY created_at DESC
             LIMIT ?"
        )
        .bind(configuration_id.to_string())
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(levels)
    }

    /// Convert to simplified format for backwards compatibility
    pub fn to_simplified(&self, channel_id: u32, is_stereo: bool) -> SimplifiedVULevelData {
        SimplifiedVULevelData {
            timestamp: self.created_at.timestamp_micros(),
            channel_id,
            peak_left: self.peak_left,
            rms_left: self.rms_left,
            peak_right: if is_stereo {
                Some(self.peak_right)
            } else {
                None
            },
            rms_right: if is_stereo {
                Some(self.rms_right)
            } else {
                None
            },
            is_stereo,
        }
    }
}

impl SimplifiedVULevelData {
    /// Convert to full VU level data format
    pub fn to_full(&self, audio_device_id: Uuid, configuration_id: Uuid) -> VULevelData {
        let timestamp = chrono::DateTime::from_timestamp_micros(self.timestamp)
            .unwrap_or_else(chrono::Utc::now);

        VULevelData {
            id: Uuid::new_v4(),
            audio_device_id,
            configuration_id,
            peak_left: self.peak_left,
            peak_right: self.peak_right.unwrap_or(0.0),
            rms_left: self.rms_left,
            rms_right: self.rms_right.unwrap_or(0.0),
            created_at: timestamp,
            updated_at: timestamp,
            deleted_at: None,
        }
    }
}
