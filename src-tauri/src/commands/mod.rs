// Command modules for better organization
pub mod application_audio;
pub mod audio_devices;
pub mod audio_effects;
pub mod debug;
pub mod file_player;
pub mod icecast;
pub mod recording;
pub mod streaming;

// Re-export all command functions for easy access
pub use application_audio::*;
pub use audio_devices::*;
pub use audio_effects::*;
pub use debug::*;
pub use file_player::*;
pub use icecast::*;
pub use recording::*;
pub use streaming::*;
