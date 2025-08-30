// Core recording types and configuration structures
//
// This module contains all the fundamental data structures for the recording
// system, including format definitions, configuration, session tracking,
// and command structures.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

/// Recording format options - matches frontend TypeScript RecordingFormat
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp3: Option<Mp3Settings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flac: Option<FlacSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wav: Option<WavSettings>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mp3Settings {
    pub bitrate: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlacSettings {
    pub compression_level: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WavSettings {
    // Empty for now, can add WAV specific settings later
}

impl Default for RecordingFormat {
    fn default() -> Self {
        RecordingFormat {
            mp3: None,
            flac: None,
            wav: Some(WavSettings {}), // Use WAV as default since MP3 has thread-safety issues
        }
    }
}

impl RecordingFormat {
    /// Get the file extension for this format
    pub fn get_file_extension(&self) -> &'static str {
        if self.mp3.is_some() {
            "mp3"
        } else if self.flac.is_some() {
            "flac"
        } else {
            "wav" // Default fallback
        }
    }
    
    /// Check if this is a lossy format
    pub fn is_lossy(&self) -> bool {
        self.mp3.is_some()
    }
    
    /// Get human-readable format name
    pub fn get_format_name(&self) -> &'static str {
        if self.mp3.is_some() {
            "MP3"
        } else if self.flac.is_some() {
            "FLAC"
        } else {
            "WAV"
        }
    }
}

/// Album artwork data for embedded images
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlbumArtwork {
    pub mime_type: String,       // e.g., "image/jpeg", "image/png"
    pub description: String,     // Description of the artwork
    pub image_data: Vec<u8>,     // Raw image bytes
    pub picture_type: ArtworkType,
}

/// Type of artwork/picture
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ArtworkType {
    Other,
    FileIcon,           // 32x32 pixels 'file icon' (PNG only)
    OtherFileIcon,
    CoverFront,         // Front cover
    CoverBack,          // Back cover  
    LeafletPage,
    Media,              // e.g. label side of CD
    LeadArtist,         // Lead artist/performer/soloist
    Artist,             // Artist/performer
    Conductor,
    Band,               // Band/Orchestra
    Composer,
    Lyricist,
    RecordingLocation,
    DuringRecording,
    DuringPerformance,
    MovieScreenCapture,
    BrightColourFish,   // A bright coloured fish
    Illustration,
    BandArtistLogotype,
    PublisherStudioLogotype,
}

impl Default for ArtworkType {
    fn default() -> Self {
        ArtworkType::CoverFront
    }
}

impl AlbumArtwork {
    /// Create new artwork from image data
    pub fn new(mime_type: String, image_data: Vec<u8>, description: String) -> Self {
        Self {
            mime_type,
            description,
            image_data,
            picture_type: ArtworkType::CoverFront,
        }
    }
    
    /// Get the file extension for the MIME type
    pub fn get_file_extension(&self) -> &str {
        match self.mime_type.as_str() {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/bmp" => "bmp",
            "image/webp" => "webp",
            _ => "jpg", // default fallback
        }
    }
    
    /// Validate that the image data matches the MIME type
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.image_data.is_empty() {
            return Err(anyhow::anyhow!("Artwork image data is empty"));
        }
        
        // Basic validation - check magic bytes for common formats
        match self.mime_type.as_str() {
            "image/jpeg" | "image/jpg" => {
                if self.image_data.len() < 3 || 
                   self.image_data[0] != 0xFF || 
                   self.image_data[1] != 0xD8 || 
                   self.image_data[2] != 0xFF {
                    return Err(anyhow::anyhow!("Invalid JPEG image data"));
                }
            },
            "image/png" => {
                if self.image_data.len() < 8 || 
                   &self.image_data[0..8] != &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
                    return Err(anyhow::anyhow!("Invalid PNG image data"));
                }
            },
            _ => {
                // For other formats, just check that data exists
            }
        }
        
        Ok(())
    }
}

/// Comprehensive audio metadata for recordings
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RecordingMetadata {
    // Core fields
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub year: Option<u16>,
    
    // Extended fields
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub track_number: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc_number: Option<u16>,
    pub total_discs: Option<u16>,
    pub copyright: Option<String>,
    pub bpm: Option<u16>,
    pub isrc: Option<String>, // International Standard Recording Code
    
    // Technical fields (auto-populated)
    pub encoder: Option<String>,
    pub encoding_date: Option<SystemTime>,
    pub sample_rate: Option<u32>,
    pub bitrate: Option<u32>,
    pub duration_seconds: Option<f64>,
    
    // Artwork
    pub artwork: Option<AlbumArtwork>,
    
    // Custom fields
    pub custom_tags: std::collections::HashMap<String, String>,
}

impl RecordingMetadata {
    /// Create metadata with just a title
    pub fn with_title(title: String) -> Self {
        Self {
            title: Some(title),
            ..Default::default()
        }
    }
    
    /// Check if metadata has any content (user-provided fields only)
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.artist.is_none() && self.album.is_none() 
            && self.genre.is_none() && self.comment.is_none() && self.year.is_none()
            && self.album_artist.is_none() && self.composer.is_none() 
            && self.track_number.is_none() && self.copyright.is_none()
            && self.bpm.is_none() && self.isrc.is_none() && self.artwork.is_none()
            && self.custom_tags.is_empty()
    }
    
    /// Set technical metadata (auto-populated during encoding)
    pub fn set_technical_metadata(&mut self, config: &RecordingConfig, encoder_name: &str) {
        self.sample_rate = Some(config.sample_rate);
        self.encoder = Some(format!("Sendin Beats v1.0 ({})", encoder_name));
        self.encoding_date = Some(SystemTime::now());
        
        // Set bitrate if available from format
        if let Some(mp3_settings) = &config.format.mp3 {
            self.bitrate = Some(mp3_settings.bitrate);
        }
    }
    
    /// Update duration when recording completes
    pub fn set_duration(&mut self, duration_seconds: f64) {
        self.duration_seconds = Some(duration_seconds);
    }
    
    /// Add custom tag
    pub fn add_custom_tag(&mut self, key: String, value: String) {
        self.custom_tags.insert(key, value);
    }
    
    /// Remove custom tag
    pub fn remove_custom_tag(&mut self, key: &str) -> Option<String> {
        self.custom_tags.remove(key)
    }
    
    /// Get all non-empty user fields as key-value pairs for display
    pub fn get_display_fields(&self) -> Vec<(String, String)> {
        let mut fields = Vec::new();
        
        if let Some(ref title) = self.title {
            fields.push(("Title".to_string(), title.clone()));
        }
        if let Some(ref artist) = self.artist {
            fields.push(("Artist".to_string(), artist.clone()));
        }
        if let Some(ref album) = self.album {
            fields.push(("Album".to_string(), album.clone()));
        }
        if let Some(ref album_artist) = self.album_artist {
            fields.push(("Album Artist".to_string(), album_artist.clone()));
        }
        if let Some(ref composer) = self.composer {
            fields.push(("Composer".to_string(), composer.clone()));
        }
        if let Some(ref genre) = self.genre {
            fields.push(("Genre".to_string(), genre.clone()));
        }
        if let Some(year) = self.year {
            fields.push(("Year".to_string(), year.to_string()));
        }
        if let Some(track_num) = self.track_number {
            let track_display = if let Some(total) = self.total_tracks {
                format!("{}/{}", track_num, total)
            } else {
                track_num.to_string()
            };
            fields.push(("Track".to_string(), track_display));
        }
        if let Some(bpm) = self.bpm {
            fields.push(("BPM".to_string(), bpm.to_string()));
        }
        if let Some(ref copyright) = self.copyright {
            fields.push(("Copyright".to_string(), copyright.clone()));
        }
        if let Some(ref comment) = self.comment {
            fields.push(("Comment".to_string(), comment.clone()));
        }
        
        // Add custom tags
        for (key, value) in &self.custom_tags {
            fields.push((key.clone(), value.clone()));
        }
        
        fields
    }
    
    /// Validate metadata fields
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate year range
        if let Some(year) = self.year {
            if year < 1900 || year > 2100 {
                return Err(anyhow::anyhow!("Year must be between 1900 and 2100"));
            }
        }
        
        // Validate BPM range
        if let Some(bpm) = self.bpm {
            if bpm == 0 || bpm > 999 {
                return Err(anyhow::anyhow!("BPM must be between 1 and 999"));
            }
        }
        
        // Validate track numbers
        if let Some(track_num) = self.track_number {
            if track_num == 0 {
                return Err(anyhow::anyhow!("Track number must be greater than 0"));
            }
            if let Some(total) = self.total_tracks {
                if track_num > total {
                    return Err(anyhow::anyhow!("Track number cannot exceed total tracks"));
                }
            }
        }
        
        // Validate artwork if present
        if let Some(ref artwork) = self.artwork {
            artwork.validate()?;
        }
        
        Ok(())
    }
}

/// Recording configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingConfig {
    pub id: String,
    pub name: String,
    pub format: RecordingFormat,
    pub output_directory: PathBuf,
    pub filename_template: String, // e.g., "{timestamp}_{title}" 
    pub metadata: RecordingMetadata,
    
    // Advanced options
    pub auto_stop_on_silence: bool,
    pub silence_threshold_db: f32,    // -60.0 dB
    pub silence_duration_sec: f32,    // 5.0 seconds
    pub max_duration_minutes: Option<u32>,
    pub max_file_size_mb: Option<u64>,
    pub split_on_interval_minutes: Option<u32>,
    
    // Quality settings
    pub sample_rate: u32,             // 48000 Hz
    pub channels: u16,                // 2 (stereo)
    pub bit_depth: u16,               // 24-bit
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "Default Recording".to_string(),
            format: RecordingFormat::default(),
            output_directory: dirs::audio_dir().unwrap_or_else(|| PathBuf::from(".")),
            filename_template: "{timestamp}_{title}".to_string(),
            metadata: RecordingMetadata::default(),
            auto_stop_on_silence: false,
            silence_threshold_db: -60.0,
            silence_duration_sec: 5.0,
            max_duration_minutes: None,
            max_file_size_mb: None,
            split_on_interval_minutes: None,
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
        }
    }
}

impl RecordingConfig {
    /// Create a new config with a specific name and format
    pub fn new(name: String, format: RecordingFormat) -> Self {
        Self {
            name,
            format,
            ..Default::default()
        }
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow::anyhow!("Recording name cannot be empty"));
        }
        
        if self.filename_template.is_empty() {
            return Err(anyhow::anyhow!("Filename template cannot be empty"));
        }
        
        if self.sample_rate < 8000 || self.sample_rate > 192000 {
            return Err(anyhow::anyhow!("Invalid sample rate: {} (must be 8000-192000)", self.sample_rate));
        }
        
        if self.channels == 0 || self.channels > 32 {
            return Err(anyhow::anyhow!("Invalid channel count: {} (must be 1-32)", self.channels));
        }
        
        if ![16, 24, 32].contains(&self.bit_depth) {
            return Err(anyhow::anyhow!("Invalid bit depth: {} (must be 16, 24, or 32)", self.bit_depth));
        }
        
        Ok(())
    }
}

/// Current recording session information with temporary file support
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingSession {
    pub id: String,
    pub config: RecordingConfig,
    pub start_time: SystemTime,
    pub current_file_path: PathBuf,        // Final destination path
    pub temp_file_path: Option<PathBuf>,   // Temporary recording file (.tmp)
    pub duration_seconds: f64,
    pub file_size_bytes: u64,
    pub is_paused: bool,
    pub is_recovering: bool,               // True if recovering from crash
    pub metadata: RecordingMetadata,       // Session-specific metadata that can be updated during recording
}

impl RecordingSession {
    /// Create a new recording session with temporary file support
    pub fn new(config: RecordingConfig, file_path: PathBuf) -> Self {
        // Create temporary file path by adding .tmp extension
        let temp_file_path = file_path.with_extension(
            format!("{}.tmp", file_path.extension().and_then(|ext| ext.to_str()).unwrap_or(""))
        );
        
        let mut session_metadata = config.metadata.clone();
        session_metadata.set_technical_metadata(&config, "WAV"); // Default to WAV, will be updated by encoder
        
        Self {
            id: Uuid::new_v4().to_string(),
            config,
            start_time: SystemTime::now(),
            current_file_path: file_path,
            temp_file_path: Some(temp_file_path),
            duration_seconds: 0.0,
            file_size_bytes: 0,
            is_paused: false,
            is_recovering: false,
            metadata: session_metadata,
        }
    }
    
    /// Create a recovery session from an existing temporary file
    pub fn recover_from_temp_file(config: RecordingConfig, final_path: PathBuf, temp_path: PathBuf, existing_duration: f64, existing_size: u64) -> Self {
        let mut session_metadata = config.metadata.clone();
        session_metadata.set_technical_metadata(&config, "WAV");
        session_metadata.set_duration(existing_duration);
        
        Self {
            id: Uuid::new_v4().to_string(),
            config,
            start_time: SystemTime::now(), // Recovery time, not original start time
            current_file_path: final_path,
            temp_file_path: Some(temp_path),
            duration_seconds: existing_duration,
            file_size_bytes: existing_size,
            is_paused: false,
            is_recovering: true,
            metadata: session_metadata,
        }
    }
    
    /// Get the file path to write to (temporary if available, otherwise final)
    pub fn get_write_path(&self) -> &PathBuf {
        self.temp_file_path.as_ref().unwrap_or(&self.current_file_path)
    }
    
    /// Finalize recording by moving temp file to final destination
    pub fn finalize_recording(&mut self) -> anyhow::Result<()> {
        if let Some(temp_path) = &self.temp_file_path {
            if temp_path.exists() {
                // Move temporary file to final destination
                std::fs::rename(temp_path, &self.current_file_path)?;
                tracing::info!("Moved temp file {} to final destination {}", 
                             temp_path.display(), self.current_file_path.display());
            }
            self.temp_file_path = None; // Clear temp path after successful move
        }
        Ok(())
    }
    
    /// Clean up temporary file if recording is cancelled
    pub fn cleanup_temp_file(&mut self) -> anyhow::Result<()> {
        if let Some(temp_path) = &self.temp_file_path {
            if temp_path.exists() {
                std::fs::remove_file(temp_path)?;
                tracing::info!("Removed temporary file: {}", temp_path.display());
            }
            self.temp_file_path = None;
        }
        Ok(())
    }
    
    /// Update session metadata during recording
    pub fn update_metadata(&mut self, metadata: RecordingMetadata) {
        // Preserve technical metadata and update user fields
        let mut updated_metadata = metadata;
        updated_metadata.sample_rate = self.metadata.sample_rate;
        updated_metadata.encoder = self.metadata.encoder.clone();
        updated_metadata.encoding_date = self.metadata.encoding_date;
        updated_metadata.bitrate = self.metadata.bitrate;
        updated_metadata.duration_seconds = self.metadata.duration_seconds;
        
        self.metadata = updated_metadata;
    }
    
    /// Get elapsed time since recording started
    pub fn get_elapsed_time(&self) -> std::time::Duration {
        self.start_time.elapsed().unwrap_or(std::time::Duration::ZERO)
    }
    
    /// Check if recording should auto-stop due to duration limit
    pub fn should_auto_stop_duration(&self) -> bool {
        if let Some(max_minutes) = self.config.max_duration_minutes {
            self.duration_seconds >= (max_minutes as f64 * 60.0)
        } else {
            false
        }
    }
    
    /// Check if recording should auto-stop due to file size limit
    pub fn should_auto_stop_size(&self) -> bool {
        if let Some(max_mb) = self.config.max_file_size_mb {
            self.file_size_bytes >= (max_mb * 1024 * 1024)
        } else {
            false
        }
    }
}

/// Recording status information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingStatus {
    pub is_recording: bool,
    pub is_paused: bool,
    #[serde(rename = "current_session")]
    pub session: Option<RecordingSession>,   // Frontend expects "current_session" 
    pub active_writers_count: usize,
    pub available_space_gb: f64,             // **RESTORED** - Frontend expects this!
    pub total_recordings: usize,             // **RESTORED** - Frontend expects this!
    pub active_recordings: Vec<String>,      // **MISSING FIELD** - Frontend expects this!
}

impl Default for RecordingStatus {
    fn default() -> Self {
        Self {
            is_recording: false,
            is_paused: false,
            session: None,
            active_writers_count: 0,
            available_space_gb: 0.0,       // **RESTORED**: Default was missing this field
            total_recordings: 0,           // **RESTORED**: Default was missing this field  
            active_recordings: vec![],     // **MISSING FIELD**: Default was missing this field
        }
    }
}

/// Historical recording entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingHistoryEntry {
    pub id: String,
    pub config_name: String,
    pub file_path: PathBuf,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub duration_seconds: f64,
    pub file_size_bytes: u64,
    pub format: RecordingFormat,
    pub metadata: RecordingMetadata,
}

impl RecordingHistoryEntry {
    /// Create a history entry from a completed session
    pub fn from_session(session: &RecordingSession, end_time: SystemTime) -> Self {
        Self {
            id: session.id.clone(),
            config_name: session.config.name.clone(),
            file_path: session.current_file_path.clone(),
            start_time: session.start_time,
            end_time,
            duration_seconds: session.duration_seconds,
            file_size_bytes: session.file_size_bytes,
            format: session.config.format.clone(),
            metadata: session.config.metadata.clone(),
        }
    }
    
    /// Get human-readable file size
    pub fn get_file_size_display(&self) -> String {
        let bytes = self.file_size_bytes as f64;
        if bytes < 1024.0 {
            format!("{:.0} B", bytes)
        } else if bytes < 1024.0 * 1024.0 {
            format!("{:.1} KB", bytes / 1024.0)
        } else if bytes < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", bytes / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes / (1024.0 * 1024.0 * 1024.0))
        }
    }
    
    /// Get human-readable duration
    pub fn get_duration_display(&self) -> String {
        let total_seconds = self.duration_seconds as u64;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        
        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}", minutes, seconds)
        }
    }
}

/// Recording commands for internal communication
#[derive(Debug)]
pub enum RecordingCommand {
    Start(RecordingConfig),
    Stop,
    Pause,
    Resume,
    UpdateMetadata(RecordingMetadata),
}

/// Configuration presets for common use cases
pub struct RecordingPresets;

impl RecordingPresets {
    /// High-quality stereo recording
    pub fn high_quality_stereo() -> RecordingConfig {
        RecordingConfig {
            name: "High Quality Stereo".to_string(),
            format: RecordingFormat {
                wav: Some(WavSettings {}),
                mp3: None,
                flac: None,
            },
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
            ..Default::default()
        }
    }
    
    /// Compressed MP3 recording
    pub fn mp3_standard() -> RecordingConfig {
        RecordingConfig {
            name: "MP3 Standard".to_string(),
            format: RecordingFormat {
                wav: None,
                mp3: Some(Mp3Settings { bitrate: 192 }),
                flac: None,
            },
            sample_rate: 44100,
            channels: 2,
            bit_depth: 16,
            ..Default::default()
        }
    }
    
    /// Lossless FLAC recording
    pub fn flac_lossless() -> RecordingConfig {
        RecordingConfig {
            name: "FLAC Lossless".to_string(),
            format: RecordingFormat {
                wav: None,
                mp3: None,
                flac: Some(FlacSettings { compression_level: 5 }),
            },
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
            ..Default::default()
        }
    }
    
    /// Podcast recording with auto-silence detection
    pub fn podcast() -> RecordingConfig {
        RecordingConfig {
            name: "Podcast Recording".to_string(),
            format: RecordingFormat {
                mp3: Some(Mp3Settings { bitrate: 128 }),
                wav: None,
                flac: None,
            },
            auto_stop_on_silence: true,
            silence_threshold_db: -45.0,
            silence_duration_sec: 3.0,
            sample_rate: 44100,
            channels: 1, // Mono for podcasts
            bit_depth: 16,
            ..Default::default()
        }
    }
    
    /// Get all available recording presets with metadata templates
    pub fn get_all_presets() -> Vec<(&'static str, RecordingConfig)> {
        vec![
            ("High Quality Stereo", Self::high_quality_stereo()),
            ("MP3 Standard", Self::mp3_standard()),
            ("FLAC Lossless", Self::flac_lossless()),
            ("Podcast", Self::podcast()),
            ("DJ Mix", Self::dj_mix()),
            ("Voice Recording", Self::voice_recording()),
            ("Live Performance", Self::live_performance()),
        ]
    }
    
    /// DJ mix recording with comprehensive metadata
    pub fn dj_mix() -> RecordingConfig {
        let mut metadata = RecordingMetadata::default();
        metadata.genre = Some("Electronic".to_string());
        metadata.album_artist = Some("Various Artists".to_string());
        metadata.comment = Some("DJ Mix recorded with Sendin Beats".to_string());
        metadata.add_custom_tag("mix_type".to_string(), "live_mix".to_string());
        metadata.add_custom_tag("equipment".to_string(), "virtual_mixer".to_string());
        
        RecordingConfig {
            name: "DJ Mix".to_string(),
            format: RecordingFormat {
                mp3: Some(Mp3Settings { bitrate: 320 }),
                wav: None,
                flac: None,
            },
            filename_template: "DJ_Mix_{timestamp}_{title}".to_string(),
            metadata,
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
            ..Default::default()
        }
    }
    
    /// Voice recording with appropriate settings
    pub fn voice_recording() -> RecordingConfig {
        let mut metadata = RecordingMetadata::default();
        metadata.genre = Some("Spoken Word".to_string());
        metadata.comment = Some("Voice recording".to_string());
        
        RecordingConfig {
            name: "Voice Recording".to_string(),
            format: RecordingFormat {
                mp3: Some(Mp3Settings { bitrate: 128 }),
                wav: None,
                flac: None,
            },
            filename_template: "Voice_{timestamp}_{title}".to_string(),
            metadata,
            sample_rate: 44100,
            channels: 1, // Mono for voice
            bit_depth: 16,
            auto_stop_on_silence: true,
            silence_threshold_db: -40.0,
            silence_duration_sec: 2.0,
            ..Default::default()
        }
    }
    
    /// Live performance recording with extended metadata
    pub fn live_performance() -> RecordingConfig {
        let mut metadata = RecordingMetadata::default();
        metadata.album = Some("Live Performance".to_string());
        metadata.comment = Some("Live performance recording".to_string());
        metadata.add_custom_tag("recording_type".to_string(), "live".to_string());
        metadata.add_custom_tag("venue".to_string(), "".to_string()); // User can fill in
        
        RecordingConfig {
            name: "Live Performance".to_string(),
            format: RecordingFormat {
                wav: Some(WavSettings {}),
                mp3: None,
                flac: None,
            },
            filename_template: "Live_{timestamp}_{artist}_{title}".to_string(),
            metadata,
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
            max_duration_minutes: Some(180), // 3 hours max
            ..Default::default()
        }
    }
}

/// Metadata templates for quick setup
pub struct MetadataPresets;

impl MetadataPresets {
    /// Get all available metadata presets
    pub fn get_all_presets() -> Vec<(&'static str, RecordingMetadata)> {
        vec![
            ("DJ Set", Self::dj_set()),
            ("Podcast Episode", Self::podcast_episode()),
            ("Music Track", Self::music_track()),
            ("Voice Memo", Self::voice_memo()),
            ("Live Mix", Self::live_mix()),
            ("Demo Recording", Self::demo_recording()),
        ]
    }
    
    /// DJ set metadata template
    pub fn dj_set() -> RecordingMetadata {
        let mut metadata = RecordingMetadata::default();
        metadata.genre = Some("Electronic".to_string());
        metadata.album_artist = Some("Various Artists".to_string());
        metadata.album = Some("DJ Set".to_string());
        metadata.comment = Some("DJ set recorded live".to_string());
        metadata.add_custom_tag("set_type".to_string(), "live_set".to_string());
        metadata.add_custom_tag("genre_primary".to_string(), "electronic".to_string());
        metadata
    }
    
    /// Podcast episode metadata template
    pub fn podcast_episode() -> RecordingMetadata {
        let mut metadata = RecordingMetadata::default();
        metadata.genre = Some("Podcast".to_string());
        metadata.album = Some("Podcast Series".to_string());
        metadata.comment = Some("Podcast episode".to_string());
        metadata.add_custom_tag("episode_type".to_string(), "full_episode".to_string());
        metadata.add_custom_tag("show_notes".to_string(), "".to_string());
        metadata
    }
    
    /// Music track metadata template  
    pub fn music_track() -> RecordingMetadata {
        let mut metadata = RecordingMetadata::default();
        metadata.track_number = Some(1);
        metadata.add_custom_tag("recording_studio".to_string(), "Home Studio".to_string());
        metadata.add_custom_tag("producer".to_string(), "".to_string());
        metadata
    }
    
    /// Voice memo metadata template
    pub fn voice_memo() -> RecordingMetadata {
        let mut metadata = RecordingMetadata::default();
        metadata.genre = Some("Voice Memo".to_string());
        metadata.comment = Some("Quick voice note".to_string());
        metadata.add_custom_tag("memo_type".to_string(), "personal".to_string());
        metadata
    }
    
    /// Live mix metadata template
    pub fn live_mix() -> RecordingMetadata {
        let mut metadata = RecordingMetadata::default();
        metadata.genre = Some("Electronic".to_string());
        metadata.album = Some("Live Mix Series".to_string());
        metadata.album_artist = Some("Various Artists".to_string());
        metadata.comment = Some("Live DJ mix with real-time effects".to_string());
        metadata.add_custom_tag("mix_style".to_string(), "continuous".to_string());
        metadata.add_custom_tag("bpm_range".to_string(), "120-140".to_string());
        metadata
    }
    
    /// Demo recording metadata template
    pub fn demo_recording() -> RecordingMetadata {
        let mut metadata = RecordingMetadata::default();
        metadata.album = Some("Demo".to_string());
        metadata.comment = Some("Demo recording for reference".to_string());
        metadata.add_custom_tag("demo_version".to_string(), "1.0".to_string());
        metadata.add_custom_tag("recording_quality".to_string(), "demo".to_string());
        metadata
    }
}