// Audio tap module - Audio tapping and system audio capture functionality  
//
// This module provides comprehensive audio tapping capabilities:
// - coreaudio_taps: Low-level CoreAudio tap implementation (macOS)
// - application_audio: Application-specific audio capture and management

#[cfg(target_os = "macos")]
pub mod coreaudio_taps;

pub mod application_audio;

// Re-export application audio types
pub use application_audio::{
    ApplicationAudioManager, ProcessInfo, TapStats, ApplicationAudioError,
};