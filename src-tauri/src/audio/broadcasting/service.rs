// Modular streaming service implementation - re-exports from submodules

// Re-export all public types and functions for backward compatibility
pub use super::config::StreamingServiceConfig;
pub use super::types::{
    ServiceState, StreamingServiceStatus, ConnectionHealth, BitrateInfo, 
    ConnectionDiagnostics, AudioStreamingStats, IcecastStreamingStats
};
pub use super::manager::StreamingService;
pub use super::utils::{
    get_streaming_service, initialize_streaming, connect_streaming_to_mixer,
    start_streaming, stop_streaming, update_stream_metadata, get_streaming_status,
    set_stream_bitrate, get_available_bitrates, get_current_stream_bitrate,
    create_stream_bitrate_preset, set_variable_bitrate_streaming, 
    get_variable_bitrate_settings
};