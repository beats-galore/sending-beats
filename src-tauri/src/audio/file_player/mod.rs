// File Player module - Audio file playback functionality
//
// This module provides comprehensive audio file playback capabilities:
// - player: Core audio file player with format support
// - manager: File player management and coordination

pub mod manager;
pub mod metadata;
pub mod player;
pub mod queue;
pub mod source;
pub mod wire;

// Re-export commonly used types from the player module
pub use player::{
    AudioFilePlayer, FilePlayerDevice, PlaybackMode, PlaybackState, PlaybackStatus, PlayerEvent,
    QueuedTrack, RepeatMode,
};

pub use metadata::{read_metadata, TrackMetadata};

// Re-export commonly used types from the manager module
pub use manager::{FilePlayerConfig, FilePlayerManager, FilePlayerService, PlaybackAction};

pub use source::{FilePlayerSource, SOURCE_CHUNK_FRAMES};
