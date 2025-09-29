// Command modules for better organization
pub mod application_audio;
pub mod audio_devices;
pub mod audio_effects;
pub mod configurations;
pub mod debug;
pub mod file_player;
pub mod icecast;
pub mod mixer;
pub mod recording;
pub mod streaming;
pub mod vu_events;

// Re-export all command functions for easy access
pub use application_audio::*;
pub use audio_devices::*;
pub use audio_effects::*;
pub use configurations::*;
pub use debug::*;
pub use file_player::*;
pub use icecast::*;
pub use mixer::*;
pub use recording::*;
pub use streaming::*;
pub use vu_events::*;
