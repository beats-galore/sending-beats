// Command modules for better organization
pub mod streaming;
pub mod audio_devices;
pub mod mixer;
pub mod audio_effects;
pub mod recording;
pub mod icecast;
pub mod debug;
pub mod file_player;
pub mod application_audio;

// Re-export all command functions for easy access
pub use streaming::*;
pub use audio_devices::*;
pub use mixer::*;
pub use audio_effects::*;
pub use recording::*;
pub use icecast::*;
pub use debug::*;
pub use file_player::*;
pub use application_audio::*;