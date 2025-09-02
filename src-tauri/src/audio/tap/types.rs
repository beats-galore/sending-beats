// Audio tap type definitions and shared structures
//
// This module provides common types and error definitions used across
// the application audio capture system.

use std::path::PathBuf;

/// Information about a discovered process that might have audio capabilities
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub bundle_id: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub is_audio_capable: bool,
    pub is_playing_audio: bool,
}

/// Helper struct for audio format information
#[derive(Debug, Clone)]
pub struct AudioFormatInfo {
    pub sample_rate: f64,
    pub channels: u32,
    pub bits_per_sample: u32,
}

/// Statistics for monitoring tap health
#[derive(Debug, Clone, serde::Serialize)]
pub struct TapStats {
    pub pid: u32,
    pub process_name: String,
    pub age: std::time::Duration,
    pub last_activity: std::time::Duration,
    pub error_count: u32,
    pub is_capturing: bool,
    pub process_alive: bool,
}

/// Context data for Core Audio tap IOProc callback
#[cfg(target_os = "macos")]
pub struct CoreAudioTapCallbackContext {
    pub audio_tx: tokio::sync::broadcast::Sender<Vec<f32>>,
    pub process_name: String,
    pub sample_rate: f64,
    pub channels: u32,
    pub callback_count: std::sync::atomic::AtomicU64,
}

/// Errors that can occur during application audio operations
#[derive(Debug, thiserror::Error)]
pub enum ApplicationAudioError {
    #[error("Permission denied - audio capture not authorized")]
    PermissionDenied,

    #[error("Application not found (PID: {pid})")]
    ApplicationNotFound { pid: u32 },

    #[error("Core Audio error: {status}")]
    CoreAudioError { status: i32 },

    #[error("Unsupported macOS version - requires 14.4+")]
    UnsupportedSystem,

    #[error("Too many active captures (max: {max})")]
    TooManyCaptures { max: usize },

    #[error("Audio tap not initialized")]
    TapNotInitialized,

    #[error("System error: {0}")]
    SystemError(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ApplicationAudioError>;
