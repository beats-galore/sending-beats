// Modular streaming service implementation - re-exports from submodules

// Re-export all public types and functions for backward compatibility
pub use super::config::StreamingServiceConfig;
pub use super::manager::StreamingService;
pub use super::types::{
    AudioStreamingStats, BitrateInfo, ConnectionDiagnostics, ConnectionHealth,
    IcecastStreamingStats, ServiceState, StreamingServiceStatus,
};
pub use super::utils::{
    connect_streaming_to_mixer, create_stream_bitrate_preset, get_available_bitrates,
    get_current_stream_bitrate, get_streaming_service, get_streaming_status,
    get_variable_bitrate_settings, initialize_streaming, set_stream_bitrate,
    set_variable_bitrate_streaming, start_streaming, start_streaming_with_consumer, stop_streaming,
    update_stream_metadata,
};
