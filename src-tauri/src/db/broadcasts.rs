use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Broadcast configuration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BroadcastConfiguration {
    pub id: Uuid,
    pub name: String,
    pub server_url: String,
    pub mount_point: String,
    pub username: String,
    pub password: String, // Should be encrypted in practice
    pub bitrate: i32,
    pub sample_rate: i32,
    pub channel_format: String,
    pub codec: String,
    pub is_variable_bitrate: bool,
    pub vbr_quality: Option<i32>,
    pub stream_name: Option<String>,
    pub stream_description: Option<String>,
    pub stream_genre: Option<String>,
    pub stream_url: Option<String>,
    pub should_auto_reconnect: bool,
    pub max_reconnect_attempts: i32,
    pub reconnect_delay_seconds: i32,
    pub connection_timeout_seconds: i32,
    pub buffer_size_ms: i32,
    pub enable_quality_monitoring: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Broadcast session
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Broadcast {
    pub id: Uuid,
    pub broadcast_config_id: Option<Uuid>,
    pub session_name: Option<String>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_seconds: Option<f64>,
    pub server_url: String,
    pub mount_point: String,
    pub stream_name: Option<String>,
    pub bitrate: i32,
    pub sample_rate: i32,
    pub channel_format: String,
    pub codec: String,
    pub actual_bitrate: Option<f64>,
    pub bytes_sent: i64,
    pub packets_sent: i64,
    pub connection_uptime_seconds: i64,
    pub reconnect_count: i32,
    pub average_bitrate_kbps: Option<f64>,
    pub packet_loss_rate: f64,
    pub latency_ms: Option<i32>,
    pub buffer_underruns: i32,
    pub encoding_errors: i32,
    pub final_status: Option<String>,
    pub last_error: Option<String>,
    pub peak_listeners: Option<i32>,
    pub total_listener_minutes: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Broadcast output chunk
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BroadcastOutput {
    pub id: Uuid,
    pub broadcast_id: Uuid,
    pub chunk_sequence: i64,
    pub chunk_timestamp: chrono::DateTime<chrono::Utc>,
    pub chunk_size_bytes: i32,
    pub encoding_duration_ms: Option<f64>,
    pub transmission_duration_ms: Option<f64>,
    pub audio_data: Option<Vec<u8>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl BroadcastConfiguration {
    /// Create new broadcast configuration
    pub fn new(
        name: String,
        server_url: String,
        mount_point: String,
        username: String,
        password: String,
        bitrate: i32,
        sample_rate: i32,
        channel_format: String,
        codec: String,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            server_url,
            mount_point,
            username,
            password,
            bitrate,
            sample_rate,
            channel_format,
            codec,
            is_variable_bitrate: false,
            vbr_quality: None,
            stream_name: None,
            stream_description: None,
            stream_genre: None,
            stream_url: None,
            should_auto_reconnect: true,
            max_reconnect_attempts: 10,
            reconnect_delay_seconds: 5,
            connection_timeout_seconds: 30,
            buffer_size_ms: 500,
            enable_quality_monitoring: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Save to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO broadcast_configurations
             (id, name, server_url, mount_point, username, password, bitrate, sample_rate,
              channel_format, codec, is_variable_bitrate, vbr_quality, stream_name,
              stream_description, stream_genre, stream_url, should_auto_reconnect,
              max_reconnect_attempts, reconnect_delay_seconds, connection_timeout_seconds,
              buffer_size_ms, enable_quality_monitoring, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(self.id.to_string())
        .bind(&self.name)
        .bind(&self.server_url)
        .bind(&self.mount_point)
        .bind(&self.username)
        .bind(&self.password)
        .bind(self.bitrate)
        .bind(self.sample_rate)
        .bind(&self.channel_format)
        .bind(&self.codec)
        .bind(self.is_variable_bitrate)
        .bind(self.vbr_quality)
        .bind(&self.stream_name)
        .bind(&self.stream_description)
        .bind(&self.stream_genre)
        .bind(&self.stream_url)
        .bind(self.should_auto_reconnect)
        .bind(self.max_reconnect_attempts)
        .bind(self.reconnect_delay_seconds)
        .bind(self.connection_timeout_seconds)
        .bind(self.buffer_size_ms)
        .bind(self.enable_quality_monitoring)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// List all active broadcast configurations
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Self>> {
        let configs = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", name, server_url, mount_point, username, password,
             bitrate, sample_rate, channel_format, codec, is_variable_bitrate, vbr_quality,
             stream_name, stream_description, stream_genre, stream_url, should_auto_reconnect,
             max_reconnect_attempts, reconnect_delay_seconds, connection_timeout_seconds,
             buffer_size_ms, enable_quality_monitoring, created_at, updated_at, deleted_at
             FROM broadcast_configurations
             WHERE deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(configs)
    }
}

impl Broadcast {
    /// Create new broadcast session
    pub fn new(
        server_url: String,
        mount_point: String,
        bitrate: i32,
        sample_rate: i32,
        channel_format: String,
        codec: String,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            broadcast_config_id: None,
            session_name: None,
            start_time: now,
            end_time: None,
            duration_seconds: None,
            server_url,
            mount_point,
            stream_name: None,
            bitrate,
            sample_rate,
            channel_format,
            codec,
            actual_bitrate: None,
            bytes_sent: 0,
            packets_sent: 0,
            connection_uptime_seconds: 0,
            reconnect_count: 0,
            average_bitrate_kbps: None,
            packet_loss_rate: 0.0,
            latency_ms: None,
            buffer_underruns: 0,
            encoding_errors: 0,
            final_status: None,
            last_error: None,
            peak_listeners: None,
            total_listener_minutes: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Save to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO broadcasts
             (id, broadcast_config_id, session_name, start_time, end_time, duration_seconds,
              server_url, mount_point, stream_name, bitrate, sample_rate, channel_format, codec,
              actual_bitrate, bytes_sent, packets_sent, connection_uptime_seconds, reconnect_count,
              average_bitrate_kbps, packet_loss_rate, latency_ms, buffer_underruns, encoding_errors,
              final_status, last_error, peak_listeners, total_listener_minutes,
              created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(self.id.to_string())
        .bind(self.broadcast_config_id.map(|id| id.to_string()))
        .bind(&self.session_name)
        .bind(self.start_time)
        .bind(self.end_time)
        .bind(self.duration_seconds)
        .bind(&self.server_url)
        .bind(&self.mount_point)
        .bind(&self.stream_name)
        .bind(self.bitrate)
        .bind(self.sample_rate)
        .bind(&self.channel_format)
        .bind(&self.codec)
        .bind(self.actual_bitrate)
        .bind(self.bytes_sent)
        .bind(self.packets_sent)
        .bind(self.connection_uptime_seconds)
        .bind(self.reconnect_count)
        .bind(self.average_bitrate_kbps)
        .bind(self.packet_loss_rate)
        .bind(self.latency_ms)
        .bind(self.buffer_underruns)
        .bind(self.encoding_errors)
        .bind(&self.final_status)
        .bind(&self.last_error)
        .bind(self.peak_listeners)
        .bind(self.total_listener_minutes)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// List all broadcasts
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Self>> {
        let broadcasts = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", broadcast_config_id as \"broadcast_config_id: Uuid?\",
             session_name, start_time, end_time, duration_seconds, server_url, mount_point,
             stream_name, bitrate, sample_rate, channel_format, codec, actual_bitrate,
             bytes_sent, packets_sent, connection_uptime_seconds, reconnect_count,
             average_bitrate_kbps, packet_loss_rate, latency_ms, buffer_underruns,
             encoding_errors, final_status, last_error, peak_listeners, total_listener_minutes,
             created_at, updated_at, deleted_at
             FROM broadcasts
             WHERE deleted_at IS NULL
             ORDER BY start_time DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(broadcasts)
    }

    /// Get active broadcasts (no end time)
    pub async fn list_active(pool: &SqlitePool) -> Result<Vec<Self>> {
        let broadcasts = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", broadcast_config_id as \"broadcast_config_id: Uuid?\",
             session_name, start_time, end_time, duration_seconds, server_url, mount_point,
             stream_name, bitrate, sample_rate, channel_format, codec, actual_bitrate,
             bytes_sent, packets_sent, connection_uptime_seconds, reconnect_count,
             average_bitrate_kbps, packet_loss_rate, latency_ms, buffer_underruns,
             encoding_errors, final_status, last_error, peak_listeners, total_listener_minutes,
             created_at, updated_at, deleted_at
             FROM broadcasts
             WHERE end_time IS NULL AND deleted_at IS NULL
             ORDER BY start_time DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(broadcasts)
    }
}
