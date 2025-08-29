// Audio mixer module - Virtual mixer with audio transformation
//
// This module provides comprehensive mixer functionality through
// a modular architecture with clear separation of concerns:
// - types: Core mixer types and VirtualMixer struct
// - validation: Security validation and input sanitization  
// - timing_synchronization: Audio clock and timing management
// - audio_processing: Real-time audio processing and level calculations
// - command_processing: Command handling and communication channels
// - stream_management: Audio stream lifecycle and device coordination
// - mixer_core: Additional core functionality and health monitoring
// - transformer: Audio format transformation and stream processing (existing)

// Core modules for mixer functionality
pub mod types;
pub mod validation;
pub mod timing_synchronization;
pub mod audio_processing;
pub mod command_processing;
pub mod stream_management;
pub mod mixer_core;

// Existing transformer module (preserved)
pub mod transformer;

// Re-export main public API
pub use types::{VirtualMixer, MixerConfigUtils};

// Re-export validation utilities
pub use validation::{validate_device_id, validate_config, SecurityUtils};

// Re-export timing synchronization types
pub use timing_synchronization::{AudioClock, TimingSync, TimingMetrics};

// Re-export stream management types
pub use stream_management::StreamInfo;

// Re-export mixer core types
pub use mixer_core::{ClockInfo, MixerStatus, HealthCheckResult, VirtualMixerHandle};

// Re-export stream management types for easier access
pub use stream_management::{AudioInputStream, AudioOutputStream};

// Re-export stream manager functions
pub use stream_management::get_stream_manager;

// Re-export stream management types
pub use stream_management::{StreamCommand, StreamManager};