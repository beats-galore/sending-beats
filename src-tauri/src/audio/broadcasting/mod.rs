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

// Re-export commonly used types from streaming
pub use streaming::{StreamManager, StreamConfig};

// Re-export service types  
pub use service::StreamingService;
pub use streaming::AudioEncoder;

// Re-export icecast types
pub use icecast_source::{IcecastSourceClient, IcecastStats, IcecastStreamManager};

// Re-export bridge types
pub use bridge::{
    AudioStreamingBridge, StreamingStatus, StreamingCommand, StreamingStats,
    create_streaming_bridge,
};