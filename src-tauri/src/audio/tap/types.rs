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
