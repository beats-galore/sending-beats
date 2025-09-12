// Audio tap module - Application-specific audio capture and management
//
// This module provides comprehensive audio tapping capabilities for capturing
// audio from specific applications on macOS. It includes process discovery,
// Core Audio tap integration, virtual stream bridging, and high-level management.
pub mod process_discovery;
pub mod types;
pub mod virtual_stream;

// Platform-specific modules
#[cfg(target_os = "macos")]
pub mod core_audio_tap;

// FFI bindings for Core Audio Taps API
#[cfg(target_os = "macos")]
pub mod core_audio_bindings;

// Re-export commonly used types
pub use types::{ApplicationAudioError, AudioFormatInfo, ProcessInfo, TapStats};

// Re-export process discovery
pub use process_discovery::ApplicationDiscovery;

// Re-export virtual stream components
pub use virtual_stream::{
    get_virtual_input_registry, ApplicationAudioInputBridge, VirtualAudioInputStream,
};

// Platform-specific re-exports
#[cfg(target_os = "macos")]
pub use core_audio_tap::ApplicationAudioTap;

#[cfg(target_os = "macos")]
pub use types::CoreAudioTapCallbackContext;

// Convenience type aliases
pub type Result<T> = std::result::Result<T, ApplicationAudioError>;
