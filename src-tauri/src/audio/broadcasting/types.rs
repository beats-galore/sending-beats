use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    pub latency_ms: Option<u64>,
    pub packet_loss_rate: f32,
    pub last_heartbeat: Option<Instant>,
    pub consecutive_failures: u32,
    pub average_bitrate_kbps: f64,
    pub buffer_underruns: u32,
}

impl Default for ConnectionHealth {
    fn default() -> Self {
        Self {
            latency_ms: None,
            packet_loss_rate: 0.0,
            last_heartbeat: None,
            consecutive_failures: 0,
            average_bitrate_kbps: 0.0,
            buffer_underruns: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub is_running: bool,
    pub is_connected: bool,
    pub is_streaming: bool,
    pub last_error: Option<String>,
    pub start_time: Option<Instant>,
    pub reconnect_attempts: u32,
    pub last_connection_time: Option<Instant>,
    pub last_disconnect_time: Option<Instant>,
    pub connection_health: ConnectionHealth,
    pub should_auto_reconnect: bool,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self {
            is_running: false,
            is_connected: false,
            is_streaming: false,
            last_error: None,
            start_time: None,
            reconnect_attempts: 0,
            last_connection_time: None,
            last_disconnect_time: None,
            connection_health: ConnectionHealth::default(),
            should_auto_reconnect: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamingServiceStatus {
    pub is_running: bool,
    pub is_connected: bool,
    pub is_streaming: bool,
    pub uptime_seconds: u64,
    pub audio_stats: Option<AudioStreamingStats>,
    pub icecast_stats: Option<IcecastStreamingStats>,
    pub connection_diagnostics: ConnectionDiagnostics,
    pub bitrate_info: BitrateInfo,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BitrateInfo {
    pub current_bitrate: u32,
    pub available_bitrates: Vec<u32>,
    pub codec: String,
    pub is_variable_bitrate: bool,
    pub vbr_quality: u8,
    pub actual_bitrate: Option<u32>, // Real-time bitrate for VBR
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionDiagnostics {
    pub latency_ms: Option<u64>,
    pub packet_loss_rate: f32,
    pub connection_stability: f32, // 0.0 to 1.0
    pub reconnect_attempts: u32,
    pub time_since_last_reconnect_seconds: Option<u64>,
    pub connection_uptime_seconds: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioStreamingStats {
    pub samples_processed: u64,
    pub samples_per_second: f64,
    pub buffer_overruns: u32,
    pub encoding_errors: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IcecastStreamingStats {
    pub bytes_sent: u64,
    pub packets_sent: u64,
    pub connection_duration_seconds: u64,
    pub average_bitrate_kbps: f64,
}
