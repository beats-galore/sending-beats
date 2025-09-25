use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Recording configuration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordingConfiguration {
    pub id: Uuid,
    pub name: String,
    pub directory: String,
    pub format: String,
    pub sample_rate: i32,
    pub bitrate: Option<i32>,
    pub filename_template: String,
    pub default_title: Option<String>,
    pub default_album: Option<String>,
    pub default_genre: Option<String>,
    pub default_artist: Option<String>,
    pub default_artwork: Option<String>,
    pub auto_stop_on_silence: bool,
    pub silence_threshold_db: Option<f64>,
    pub max_file_size_mb: Option<i32>,
    pub split_on_interval_minutes: Option<i32>,
    pub channel_format: String,
    pub bit_depth: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Recording entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Recording {
    pub id: Uuid,
    pub recording_config_id: Option<Uuid>,
    pub internal_directory: String,
    pub file_name: String,
    pub size_mb: f64,
    pub format: String,
    pub sample_rate: i32,
    pub bitrate: Option<i32>,
    pub duration_seconds: f64,
    pub channel_format: String,
    pub bit_depth: i32,

    // Metadata fields
    pub title: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub artist: Option<String>,
    pub artwork: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub track_number: Option<i32>,
    pub total_tracks: Option<i32>,
    pub disc_number: Option<i32>,
    pub total_discs: Option<i32>,
    pub copyright: Option<String>,
    pub bpm: Option<i32>,
    pub isrc: Option<String>,
    pub encoder: Option<String>,
    pub encoding_date: Option<chrono::DateTime<chrono::Utc>>,
    pub comment: Option<String>,
    pub year: Option<i32>,

    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Recording output chunk
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordingOutput {
    pub id: Uuid,
    pub recording_id: Uuid,
    pub chunk_sequence: i32,
    pub output_data: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl RecordingConfiguration {
    /// Create new recording configuration
    pub fn new(
        name: String,
        directory: String,
        format: String,
        sample_rate: i32,
        channel_format: String,
        bit_depth: i32,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            directory,
            format,
            sample_rate,
            bitrate: None,
            filename_template: "{timestamp}_{title}".to_string(),
            default_title: None,
            default_album: None,
            default_genre: None,
            default_artist: None,
            default_artwork: None,
            auto_stop_on_silence: false,
            silence_threshold_db: None,
            max_file_size_mb: None,
            split_on_interval_minutes: None,
            channel_format,
            bit_depth,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Save to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO recording_configurations
             (id, name, directory, format, sample_rate, bitrate, filename_template,
              default_title, default_album, default_genre, default_artist, default_artwork,
              auto_stop_on_silence, silence_threshold_db, max_file_size_mb, split_on_interval_minutes,
              channel_format, bit_depth, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(self.id.to_string())
        .bind(&self.name)
        .bind(&self.directory)
        .bind(&self.format)
        .bind(self.sample_rate)
        .bind(self.bitrate)
        .bind(&self.filename_template)
        .bind(&self.default_title)
        .bind(&self.default_album)
        .bind(&self.default_genre)
        .bind(&self.default_artist)
        .bind(&self.default_artwork)
        .bind(self.auto_stop_on_silence)
        .bind(self.silence_threshold_db)
        .bind(self.max_file_size_mb)
        .bind(self.split_on_interval_minutes)
        .bind(&self.channel_format)
        .bind(self.bit_depth)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// List all active recording configurations
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Self>> {
        let configs = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", name, directory, format, sample_rate, bitrate, filename_template,
             default_title, default_album, default_genre, default_artist, default_artwork,
             auto_stop_on_silence, silence_threshold_db, max_file_size_mb, split_on_interval_minutes,
             channel_format, bit_depth, created_at, updated_at, deleted_at
             FROM recording_configurations
             WHERE deleted_at IS NULL
             ORDER BY created_at DESC"
        )
        .fetch_all(pool)
        .await?;

        Ok(configs)
    }
}

impl Recording {
    /// Create new recording
    pub fn new(
        internal_directory: String,
        file_name: String,
        size_mb: f64,
        format: String,
        sample_rate: i32,
        duration_seconds: f64,
        channel_format: String,
        bit_depth: i32,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            recording_config_id: None,
            internal_directory,
            file_name,
            size_mb,
            format,
            sample_rate,
            bitrate: None,
            duration_seconds,
            channel_format,
            bit_depth,
            title: None,
            album: None,
            genre: None,
            artist: None,
            artwork: None,
            album_artist: None,
            composer: None,
            track_number: None,
            total_tracks: None,
            disc_number: None,
            total_discs: None,
            copyright: None,
            bpm: None,
            isrc: None,
            encoder: None,
            encoding_date: None,
            comment: None,
            year: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Save to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO recordings
             (id, recording_config_id, internal_directory, file_name, size_mb, format,
              sample_rate, bitrate, duration_seconds, channel_format, bit_depth,
              title, album, genre, artist, artwork, album_artist, composer,
              track_number, total_tracks, disc_number, total_discs, copyright, bpm, isrc,
              encoder, encoding_date, comment, year, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(self.id.to_string())
        .bind(self.recording_config_id.map(|id| id.to_string()))
        .bind(&self.internal_directory)
        .bind(&self.file_name)
        .bind(self.size_mb)
        .bind(&self.format)
        .bind(self.sample_rate)
        .bind(self.bitrate)
        .bind(self.duration_seconds)
        .bind(&self.channel_format)
        .bind(self.bit_depth)
        .bind(&self.title)
        .bind(&self.album)
        .bind(&self.genre)
        .bind(&self.artist)
        .bind(&self.artwork)
        .bind(&self.album_artist)
        .bind(&self.composer)
        .bind(self.track_number)
        .bind(self.total_tracks)
        .bind(self.disc_number)
        .bind(self.total_discs)
        .bind(&self.copyright)
        .bind(self.bpm)
        .bind(&self.isrc)
        .bind(&self.encoder)
        .bind(self.encoding_date)
        .bind(&self.comment)
        .bind(self.year)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// List all active recordings
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Self>> {
        let recordings = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", recording_config_id as \"recording_config_id: Uuid?\",
             internal_directory, file_name, size_mb, format, sample_rate, bitrate,
             duration_seconds, channel_format, bit_depth, title, album, genre, artist,
             artwork, album_artist, composer, track_number, total_tracks, disc_number,
             total_discs, copyright, bpm, isrc, encoder, encoding_date, comment, year,
             created_at, updated_at, deleted_at
             FROM recordings
             WHERE deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(recordings)
    }
}

impl RecordingOutput {
    /// Create new recording output chunk
    pub fn new(recording_id: Uuid, chunk_sequence: i32, output_data: Vec<u8>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            recording_id,
            chunk_sequence,
            output_data,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Save to database
    pub async fn save(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "INSERT INTO recording_output
             (id, recording_id, chunk_sequence, output_data, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(self.id.to_string())
        .bind(self.recording_id.to_string())
        .bind(self.chunk_sequence)
        .bind(&self.output_data)
        .bind(self.created_at)
        .bind(self.updated_at)
        .bind(self.deleted_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get all chunks for a recording
    pub async fn get_for_recording(pool: &SqlitePool, recording_id: Uuid) -> Result<Vec<Self>> {
        let chunks = sqlx::query_as::<_, Self>(
            "SELECT id as \"id: Uuid\", recording_id as \"recording_id: Uuid\",
             chunk_sequence, output_data, created_at, updated_at, deleted_at
             FROM recording_output
             WHERE recording_id = ? AND deleted_at IS NULL
             ORDER BY chunk_sequence ASC",
        )
        .bind(recording_id.to_string())
        .fetch_all(pool)
        .await?;

        Ok(chunks)
    }
}
