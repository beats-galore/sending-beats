// Audio tap module - Application-specific audio capture and management
//
// This module provides comprehensive audio tapping capabilities for capturing
// audio from specific applications on macOS. It includes process discovery,
// Core Audio tap integration, virtual stream bridging, and high-level management.
pub mod manager;
pub mod process_discovery;
pub mod types;

// Re-export commonly used types
pub use types::{ProcessInfo, TapStats};

// Re-export process discovery
pub use process_discovery::ApplicationDiscovery;

pub use manager::ApplicationAudioManager;
