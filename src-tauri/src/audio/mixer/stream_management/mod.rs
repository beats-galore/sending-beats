// Stream management module - Audio stream lifecycle and device coordination
//
// This module has been split into separate files for better organization:
// - isolated_audio_manager: Main audio processing manager with event-driven architecture
// - stream_manager: Hardware stream management for CoreAudio/CPAL integration

pub mod isolated_audio_manager;
pub mod stream_manager;
pub mod virtual_mixer;


pub use isolated_audio_manager::{AudioCommand, IsolatedAudioManager};
pub use stream_manager::{AudioMetrics, StreamInfo, StreamManager};
pub use virtual_mixer::VirtualMixer;