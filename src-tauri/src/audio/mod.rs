// Audio module - Modularized audio system for Sweet Beats Studio
//
// This module provides a comprehensive audio processing system broken down into logical components:
// - types: Core data types and configurations
// - devices: Audio device management and enumeration
// - effects: Audio effects processing (EQ, compression, limiting, analysis)
// - streams: Audio stream management (input/output)
// - mixer: Core virtual mixer functionality

pub mod broadcasting;
pub mod devices;
pub mod effects;
pub mod events;
pub mod file_player;
pub mod mixer;
pub mod now_playing;
pub mod recording;
pub mod tap;
pub mod types;
pub mod vu_channel_service;

#[cfg(target_os = "macos")]
pub mod screencapture;

// Re-export commonly used types for easier imports
pub use types::{
    AudioChannel, AudioConfigFactory, AudioDeviceHandle, AudioDeviceInfo, AudioMetrics, MixerConfig,
};

#[cfg(target_os = "macos")]
pub use types::CoreAudioDevice;

pub use effects::{
    AudioAnalyzer, BiquadFilter, Compressor, CustomAudioEffectsChain, EQBand, Limiter,
    PeakDetector, RmsDetector, SpectrumAnalyzer, ThreeBandEqualizer,
};

pub use crate::db::AudioDatabase;

pub use broadcasting::StreamManager;

pub use devices::AudioDeviceManager;

pub use recording::{
    RecordingCommand, RecordingConfig, RecordingFormat, RecordingHistoryEntry, RecordingMetadata,
    RecordingService, RecordingSession, RecordingStatus, RecordingWriter,
};

pub use tap::{ApplicationAudioManager, ApplicationDiscovery, ProcessInfo, TapStats};

pub use file_player::{
    read_metadata, AudioFilePlayer, FilePlayerConfig, FilePlayerDevice, FilePlayerManager,
    FilePlayerService, PlaybackAction, PlaybackMode, PlaybackState, PlaybackStatus, PlayerEvent,
    QueuedTrack, RepeatMode, TrackMetadata,
};

pub use events::{MasterVULevelEvent, VUChannelData, VULevelEvent};
pub use vu_channel_service::{new_shared_vu_channel, SharedVUChannel, VUChannelService};
