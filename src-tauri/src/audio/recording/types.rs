// Core recording types and configuration structures
//
// This module contains all the fundamental data structures for the recording
// system, including format definitions, configuration, session tracking,
// and command structures.

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

/// Audio metadata for recordings
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RecordingMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub year: Option<u16>,
}

impl RecordingMetadata {
    /// Create metadata with just a title
    pub fn with_title(title: String) -> Self {
        Self {
            title: Some(title),
            ..Default::default()
        }
    }
    
    /// Check if metadata has any content
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.artist.is_none() && self.album.is_none() 
            && self.genre.is_none() && self.comment.is_none() && self.year.is_none()
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

/// Current recording session information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingSession {
    pub id: String,
    pub config: RecordingConfig,
    pub start_time: SystemTime,
    pub current_file_path: PathBuf,
    pub duration_seconds: f64,
    pub file_size_bytes: u64,
    pub is_paused: bool,
}

impl RecordingSession {
    /// Create a new recording session
    pub fn new(config: RecordingConfig, file_path: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            config,
            start_time: SystemTime::now(),
            current_file_path: file_path,
            duration_seconds: 0.0,
            file_size_bytes: 0,
            is_paused: false,
        }
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
}