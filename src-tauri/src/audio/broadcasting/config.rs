use super::icecast_source::{AudioCodec, AudioFormat};
use crate::types::DEFAULT_SAMPLE_RATE;

#[derive(Debug, Clone)]
pub struct StreamingServiceConfig {
    /// Icecast server configuration
    pub server_host: String,
    pub server_port: u16,
    pub mount_point: String,
    pub password: String,

    /// Stream metadata
    pub stream_name: String,
    pub stream_description: String,
    pub stream_genre: String,
    pub stream_url: String,
    pub is_public: bool,

    /// Audio encoding settings
    pub audio_format: AudioFormat,

    /// Available bitrate presets
    pub available_bitrates: Vec<u32>,
    pub selected_bitrate: u32,

    /// Variable bitrate settings
    pub enable_variable_bitrate: bool,
    pub vbr_quality: u8, // VBR quality level 0-9 (MP3 standard)

    /// Advanced settings
    pub auto_reconnect: bool,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
}

impl Default for StreamingServiceConfig {
    fn default() -> Self {
        Self {
            server_host: "localhost".to_string(),
            server_port: 8000,
            mount_point: "/live".to_string(),
            password: std::env::var("ICECAST_PASSWORD").unwrap_or_else(|_| "changeme".to_string()),
            stream_name: "Sendin Beats Radio".to_string(),
            stream_description: "Live radio stream from Sendin Beats".to_string(),
            stream_genre: "Electronic".to_string(),
            stream_url: "https://sendinbeats.com".to_string(),
            is_public: true,
            audio_format: AudioFormat {
                sample_rate: DEFAULT_SAMPLE_RATE,
                channels: 2,
                bitrate: 192,
                codec: AudioCodec::Mp3,
            },
            available_bitrates: vec![96, 128, 160, 192, 256, 320],
            selected_bitrate: 192,
            enable_variable_bitrate: false,
            vbr_quality: 2, // High quality (V2) by default
            auto_reconnect: true,
            max_reconnect_attempts: 5,
            reconnect_delay_ms: 3000,
        }
    }
}
