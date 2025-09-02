use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use std::path::Path;
use std::sync::Arc;

/// VU meter level data for real-time buffering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VULevelData {
    pub timestamp: i64, // Microseconds since Unix epoch
    pub channel_id: u32,
    pub peak_left: f32,
    pub rms_left: f32,
    pub peak_right: Option<f32>, // None for mono sources
    pub rms_right: Option<f32>,  // None for mono sources
    pub is_stereo: bool,
}

/// Master output level data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterLevelData {
    pub timestamp: i64,
    pub peak_left: f32,
    pub rms_left: f32,
    pub peak_right: f32,
    pub rms_right: f32,
}

/// Audio device configuration
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

/// Channel configuration with all mixer settings
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

/// Output routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputRouteConfig {
    pub id: Option<u32>,
    pub name: String,
    pub output_device_id: String,
    pub gain: f32,
    pub enabled: bool,
    pub is_master: bool,
}

/// SQLite-based database manager for audio system
pub struct AudioDatabase {
    pool: SqlitePool,
    retention_seconds: i64,
}

impl AudioDatabase {
    /// Initialize the database with automatic migrations
    pub async fn new(database_path: &Path) -> Result<Self> {
        println!(
            "🗄️  Initializing SQLite database at: {}",
            database_path.display()
        );

        // Ensure parent directory exists
        if let Some(parent) = database_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create database directory")?;
        }

        // Create connection pool with SQLite-specific options
        let database_url = format!("sqlite:{}?mode=rwc", database_path.display());
        println!("🗄️  Database URL: {}", database_url);

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .context("Failed to connect to SQLite database")?;

        println!(
            "✅ SQLite connection pool created with {} max connections",
            10
        );

        // Run migrations
        println!("🔄 Running database migrations...");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context("Failed to run database migrations")?;

        println!("✅ Database migrations completed successfully");

        // Get VU retention setting
        let retention_seconds = Self::get_vu_retention_seconds(&pool).await.unwrap_or(60); // Default to 60 seconds

        println!("📊 VU level retention set to {} seconds", retention_seconds);

        Ok(Self {
            pool,
            retention_seconds,
        })
    }

    /// Get VU level retention from settings
    async fn get_vu_retention_seconds(pool: &SqlitePool) -> Result<i64> {
        let row = sqlx::query("SELECT value FROM mixer_settings WHERE key = ?")
            .bind("vu_retention_seconds")
            .fetch_one(pool)
            .await?;

        let value: String = row.get("value");
        value.parse::<i64>().context("Invalid retention setting")
    }

    /// Insert VU level data (high-frequency, optimized for real-time)
    pub async fn insert_vu_levels(&self, levels: &[VULevelData]) -> Result<()> {
        if levels.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for level in levels {
            sqlx::query(
                "INSERT OR REPLACE INTO vu_levels 
                 (timestamp, channel_id, peak_left, rms_left, peak_right, rms_right, is_stereo)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(level.timestamp)
            .bind(level.channel_id as i64)
            .bind(level.peak_left)
            .bind(level.rms_left)
            .bind(level.peak_right.unwrap_or(0.0))
            .bind(level.rms_right.unwrap_or(0.0))
            .bind(level.is_stereo)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Insert master level data
    pub async fn insert_master_levels(&self, levels: &[MasterLevelData]) -> Result<()> {
        if levels.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for level in levels {
            sqlx::query(
                "INSERT OR REPLACE INTO master_levels 
                 (timestamp, peak_left, rms_left, peak_right, rms_right)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(level.timestamp)
            .bind(level.peak_left)
            .bind(level.rms_left)
            .bind(level.peak_right)
            .bind(level.rms_right)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get recent VU levels for a channel
    pub async fn get_recent_vu_levels(
        &self,
        channel_id: u32,
        limit: i64,
    ) -> Result<Vec<VULevelData>> {
        let rows = sqlx::query(
            "SELECT timestamp, channel_id, peak_left, rms_left, peak_right, rms_right, is_stereo
             FROM vu_levels 
             WHERE channel_id = ?
             ORDER BY timestamp DESC 
             LIMIT ?",
        )
        .bind(channel_id as i64)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut levels = Vec::new();
        for row in rows {
            let is_stereo: bool = row.get("is_stereo");
            levels.push(VULevelData {
                timestamp: row.get("timestamp"),
                channel_id: row.get::<i64, _>("channel_id") as u32,
                peak_left: row.get("peak_left"),
                rms_left: row.get("rms_left"),
                peak_right: if is_stereo {
                    Some(row.get("peak_right"))
                } else {
                    None
                },
                rms_right: if is_stereo {
                    Some(row.get("rms_right"))
                } else {
                    None
                },
                is_stereo,
            });
        }

        Ok(levels)
    }

    /// Get recent master levels
    pub async fn get_recent_master_levels(&self, limit: i64) -> Result<Vec<MasterLevelData>> {
        let rows = sqlx::query(
            "SELECT timestamp, peak_left, rms_left, peak_right, rms_right
             FROM master_levels 
             ORDER BY timestamp DESC 
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut levels = Vec::new();
        for row in rows {
            levels.push(MasterLevelData {
                timestamp: row.get("timestamp"),
                peak_left: row.get("peak_left"),
                rms_left: row.get("rms_left"),
                peak_right: row.get("peak_right"),
                rms_right: row.get("rms_right"),
            });
        }

        Ok(levels)
    }

    /// Save channel configuration
    pub async fn save_channel_config(&self, channel: &ChannelConfig) -> Result<u32> {
        let now = chrono::Utc::now().timestamp();

        if let Some(id) = channel.id {
            // Update existing channel
            sqlx::query(
                "UPDATE channels SET 
                 name = ?, input_device_id = ?, gain = ?, pan = ?, muted = ?, solo = ?,
                 effects_enabled = ?, eq_low_gain = ?, eq_mid_gain = ?, eq_high_gain = ?,
                 comp_enabled = ?, comp_threshold = ?, comp_ratio = ?, comp_attack = ?, comp_release = ?,
                 limiter_enabled = ?, limiter_threshold = ?, updated_at = ?
                 WHERE id = ?"
            )
            .bind(&channel.name)
            .bind(&channel.input_device_id)
            .bind(channel.gain)
            .bind(channel.pan)
            .bind(channel.muted)
            .bind(channel.solo)
            .bind(channel.effects_enabled)
            .bind(channel.eq_low_gain)
            .bind(channel.eq_mid_gain)
            .bind(channel.eq_high_gain)
            .bind(channel.comp_enabled)
            .bind(channel.comp_threshold)
            .bind(channel.comp_ratio)
            .bind(channel.comp_attack)
            .bind(channel.comp_release)
            .bind(channel.limiter_enabled)
            .bind(channel.limiter_threshold)
            .bind(now)
            .bind(id as i64)
            .execute(&self.pool)
            .await?;

            Ok(id)
        } else {
            // Insert new channel
            let result = sqlx::query(
                "INSERT INTO channels 
                 (name, input_device_id, gain, pan, muted, solo, effects_enabled,
                  eq_low_gain, eq_mid_gain, eq_high_gain, comp_enabled, comp_threshold,
                  comp_ratio, comp_attack, comp_release, limiter_enabled, limiter_threshold)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&channel.name)
            .bind(&channel.input_device_id)
            .bind(channel.gain)
            .bind(channel.pan)
            .bind(channel.muted)
            .bind(channel.solo)
            .bind(channel.effects_enabled)
            .bind(channel.eq_low_gain)
            .bind(channel.eq_mid_gain)
            .bind(channel.eq_high_gain)
            .bind(channel.comp_enabled)
            .bind(channel.comp_threshold)
            .bind(channel.comp_ratio)
            .bind(channel.comp_attack)
            .bind(channel.comp_release)
            .bind(channel.limiter_enabled)
            .bind(channel.limiter_threshold)
            .execute(&self.pool)
            .await?;

            Ok(result.last_insert_rowid() as u32)
        }
    }

    /// Load all channel configurations
    pub async fn load_channel_configs(&self) -> Result<Vec<ChannelConfig>> {
        let rows = sqlx::query("SELECT * FROM channels ORDER BY id")
            .fetch_all(&self.pool)
            .await?;

        let mut channels = Vec::new();
        for row in rows {
            channels.push(ChannelConfig {
                id: Some(row.get::<i64, _>("id") as u32),
                name: row.get("name"),
                input_device_id: row.get("input_device_id"),
                gain: row.get("gain"),
                pan: row.get("pan"),
                muted: row.get("muted"),
                solo: row.get("solo"),
                effects_enabled: row.get("effects_enabled"),
                eq_low_gain: row.get("eq_low_gain"),
                eq_mid_gain: row.get("eq_mid_gain"),
                eq_high_gain: row.get("eq_high_gain"),
                comp_enabled: row.get("comp_enabled"),
                comp_threshold: row.get("comp_threshold"),
                comp_ratio: row.get("comp_ratio"),
                comp_attack: row.get("comp_attack"),
                comp_release: row.get("comp_release"),
                limiter_enabled: row.get("limiter_enabled"),
                limiter_threshold: row.get("limiter_threshold"),
            });
        }

        Ok(channels)
    }

    /// Save audio device configuration
    pub async fn save_audio_device(&self, device: &AudioDeviceConfig) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO audio_devices 
             (id, name, device_type, sample_rate, channels, is_default, is_active, last_seen)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&device.id)
        .bind(&device.name)
        .bind(&device.device_type)
        .bind(device.sample_rate as i64)
        .bind(device.channels as i64)
        .bind(device.is_default)
        .bind(device.is_active)
        .bind(device.last_seen)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Cleanup old VU level data to prevent database growth
    pub async fn cleanup_old_vu_levels(&self) -> Result<u64> {
        let cutoff_timestamp =
            chrono::Utc::now().timestamp_micros() - (self.retention_seconds * 1_000_000);

        let result = sqlx::query("DELETE FROM vu_levels WHERE timestamp < ?")
            .bind(cutoff_timestamp)
            .execute(&self.pool)
            .await?;

        let deleted_count = result.rows_affected();

        if deleted_count > 0 {
            // Also cleanup master levels
            let master_result = sqlx::query("DELETE FROM master_levels WHERE timestamp < ?")
                .bind(cutoff_timestamp)
                .execute(&self.pool)
                .await?;

            println!(
                "🧹 Cleaned up {} VU level records and {} master level records older than {}s",
                deleted_count,
                master_result.rows_affected(),
                self.retention_seconds
            );
        }

        Ok(deleted_count)
    }

    /// Get database connection pool for advanced queries
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// Lock-free audio event bus for real-time VU meter data
pub struct AudioEventBus {
    vu_events: Arc<crossbeam::queue::SegQueue<VULevelData>>,
    master_events: Arc<crossbeam::queue::SegQueue<MasterLevelData>>,
    max_queue_size: usize,
}

impl AudioEventBus {
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            vu_events: Arc::new(crossbeam::queue::SegQueue::new()),
            master_events: Arc::new(crossbeam::queue::SegQueue::new()),
            max_queue_size,
        }
    }

    /// Push VU level data (real-time safe, lock-free)
    pub fn push_vu_levels(&self, level: VULevelData) {
        // Prevent queue from growing too large
        while self.vu_events.len() >= self.max_queue_size {
            self.vu_events.pop();
        }

        self.vu_events.push(level);
    }

    /// Push master level data (real-time safe, lock-free)
    pub fn push_master_levels(&self, level: MasterLevelData) {
        // Prevent queue from growing too large
        while self.master_events.len() >= self.max_queue_size {
            self.master_events.pop();
        }

        self.master_events.push(level);
    }

    /// Drain all VU level events
    pub fn drain_vu_events(&self) -> Vec<VULevelData> {
        let mut events = Vec::new();
        while let Some(event) = self.vu_events.pop() {
            events.push(event);
        }
        events
    }

    /// Drain all master level events
    pub fn drain_master_events(&self) -> Vec<MasterLevelData> {
        let mut events = Vec::new();
        while let Some(event) = self.master_events.pop() {
            events.push(event);
        }
        events
    }
}
