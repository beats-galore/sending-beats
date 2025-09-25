use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Audio mixer configuration - parent mapping table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AudioMixerConfiguration {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub configuration_type: String, // 'reusable' or 'session'
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl AudioMixerConfiguration {
    /// Create a new mixer configuration
    pub fn new(name: String, configuration_type: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description: None,
            configuration_type,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Create a new session-type configuration
    pub fn new_session(name: String) -> Self {
        Self::new(name, "session".to_string())
    }

    /// Create a new reusable configuration
    pub fn new_reusable(name: String) -> Self {
        Self::new(name, "reusable".to_string())
    }

    /// Save configuration to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO audio_mixer_configurations
             (id, name, description, configuration_type, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(self.id.to_string())
        .bind(&self.name)
        .bind(&self.description)
        .bind(&self.configuration_type)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update configuration in database
    pub async fn update(&mut self, pool: &SqlitePool) -> Result<()> {
        self.updated_at = chrono::Utc::now();

        sqlx::query(
            "UPDATE audio_mixer_configurations
             SET name = ?, description = ?, configuration_type = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&self.name)
        .bind(&self.description)
        .bind(&self.configuration_type)
        .bind(self.updated_at)
        .bind(self.id.to_string())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Soft delete configuration
    pub async fn delete(&mut self, pool: &SqlitePool) -> Result<()> {
        self.deleted_at = Some(chrono::Utc::now());

        sqlx::query(
            "UPDATE audio_mixer_configurations
             SET deleted_at = ?
             WHERE id = ?",
        )
        .bind(self.deleted_at)
        .bind(self.id.to_string())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Find configuration by ID
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let config = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", name, description, configuration_type,
             created_at, updated_at, deleted_at
             FROM audio_mixer_configurations
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;

        Ok(config)
    }

    /// List all active configurations
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Self>> {
        let configs = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", name, description, configuration_type,
             created_at, updated_at, deleted_at
             FROM audio_mixer_configurations
             WHERE deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(configs)
    }

    /// List configurations by type
    pub async fn list_by_type(pool: &SqlitePool, config_type: &str) -> Result<Vec<Self>> {
        let configs = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", name, description, configuration_type,
             created_at, updated_at, deleted_at
             FROM audio_mixer_configurations
             WHERE configuration_type = ? AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(config_type)
        .fetch_all(pool)
        .await?;

        Ok(configs)
    }

    /// List only reusable configurations
    pub async fn list_reusable(pool: &SqlitePool) -> Result<Vec<Self>> {
        Self::list_by_type(pool, "reusable").await
    }

    /// List only session configurations
    pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<Self>> {
        Self::list_by_type(pool, "session").await
    }
}
