// Audio broadcasting module - Icecast streaming and broadcasting functionality
//
// This module provides comprehensive audio broadcasting capabilities:
// - streaming: Core streaming functionality and protocols
// - service: Broadcasting service management and coordination
// - icecast_source: Icecast-specific source implementation
// - bridge: Audio streaming bridge connecting mixer to broadcast

pub mod bridge;
pub mod icecast_source;
pub mod send_loop;
pub mod service;
pub mod streaming;

// Modular service components
pub mod config;
pub mod manager;
pub mod types;
pub mod utils;

// The second transmitter, and the parts both of them share
pub mod impulse;
pub mod metadata;
pub mod on_air;
pub mod protocol;
pub mod status;

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
pub use streaming::AudioEncoder;

// Re-export icecast types
pub use icecast_source::{IcecastSourceClient, IcecastStats, IcecastStreamManager};

// Re-export bridge types
pub use bridge::{
    create_streaming_bridge, AudioStreamingBridge, StreamingCommand, StreamingStats,
    StreamingStatus,
};

// Re-export the Impulse transmitter and what both transmitters share
pub use impulse::{get_impulse_service, ImpulseConfig, ImpulseService, ImpulseStats};
pub use protocol::CastProtocol;
pub use status::cast_status;
pub use types::ImpulseStreamingStats;
