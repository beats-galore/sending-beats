// Audio broadcasting module - Icecast streaming and broadcasting functionality
//
// This module provides comprehensive audio broadcasting capabilities:
// - streaming: Core streaming functionality and protocols
// - service: Broadcasting service management and coordination
// - icecast_source: Icecast-specific source implementation
// - bridge: Audio streaming bridge connecting mixer to broadcast

pub mod streaming;
pub mod service;
pub mod icecast_source;
pub mod bridge;

// Modular service components
pub mod config;
pub mod types;
pub mod manager;
pub mod utils;

// Re-export commonly used types from streaming
pub use streaming::{StreamManager, StreamConfig};

// Re-export service types and all modularized components
pub use service::{
    StreamingService, StreamingServiceConfig,
    ServiceState, StreamingServiceStatus, ConnectionHealth, BitrateInfo, 
    ConnectionDiagnostics, AudioStreamingStats, IcecastStreamingStats,
    get_streaming_service, initialize_streaming, connect_streaming_to_mixer,
    start_streaming, stop_streaming, update_stream_metadata, get_streaming_status,
    set_stream_bitrate, get_available_bitrates, get_current_stream_bitrate,
    create_stream_bitrate_preset, set_variable_bitrate_streaming, 
    get_variable_bitrate_settings
};
pub use streaming::AudioEncoder;

// Re-export icecast types
pub use icecast_source::{IcecastSourceClient, IcecastStats, IcecastStreamManager};

// Re-export bridge types
pub use bridge::{
    AudioStreamingBridge, StreamingStatus, StreamingCommand, StreamingStats,
    create_streaming_bridge,
};