// Audio broadcasting module - Icecast streaming and broadcasting functionality
//
// This module provides comprehensive audio broadcasting capabilities:
// - streaming: Core streaming functionality and protocols
// - service: Broadcasting service management and coordination
// - icecast_source: Icecast-specific source implementation

pub mod icecast_source;
pub mod service;
pub mod streaming;

// Modular service components
pub mod config;
pub mod manager;
pub mod types;
pub mod utils;

// Re-export commonly used types from streaming
pub use streaming::{StreamConfig, StreamManager};

// Re-export service types and all modularized components
pub use service::{
    create_stream_bitrate_preset, get_available_bitrates, get_current_stream_bitrate,
    get_streaming_service, get_streaming_status, get_variable_bitrate_settings, set_stream_bitrate,
    set_variable_bitrate_streaming, start_streaming, stop_streaming, update_stream_metadata,
    AudioStreamingStats, BitrateInfo, ConnectionDiagnostics, ConnectionHealth,
    IcecastStreamingStats, ServiceState, StreamingService, StreamingServiceConfig,
    StreamingServiceStatus,
};
// Re-export icecast types
pub use icecast_source::{IcecastSourceClient, IcecastStats, IcecastStreamManager};
