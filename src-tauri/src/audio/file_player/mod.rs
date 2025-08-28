// File Player module - Audio file playback functionality
//
// This module provides comprehensive audio file playback capabilities:
// - player: Core audio file player with format support
// - manager: File player management and coordination

pub mod player;
pub mod manager;

// Re-export commonly used types from the player module
pub use player::{
    AudioFilePlayer, FilePlayerDevice, QueuedTrack, PlaybackState, PlaybackStatus, PlaybackMode, RepeatMode,
};

// Re-export commonly used types from the manager module  
pub use manager::{
    FilePlayerManager, FilePlayerService, FilePlayerConfig, PlaybackAction,
};