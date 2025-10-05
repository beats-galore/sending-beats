// Audio module - Modularized audio system for Sending Beats
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
pub mod manager;
pub mod mixer;
pub mod recording;
pub mod tap;
pub mod types;
pub mod utils;
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

pub use mixer::stream_management::VirtualMixer;

pub use crate::db::AudioDatabase;

pub use broadcasting::{
    create_streaming_bridge, AudioEncoder, AudioStreamingBridge, IcecastSourceClient, IcecastStats,
    IcecastStreamManager, StreamConfig, StreamManager, StreamingCommand, StreamingService,
    StreamingStats, StreamingStatus,
};

pub use devices::{
    get_device_monitor, get_device_monitoring_stats, AudioDeviceManager, DeviceMonitor,
    DeviceMonitorConfig, DeviceMonitorStats,
};

pub use recording::{
    RecordingCommand, RecordingConfig, RecordingFormat, RecordingHistoryEntry, RecordingMetadata,
    RecordingService, RecordingSession, RecordingStatus, RecordingWriter,
};

pub use tap::{
    get_virtual_input_registry, ApplicationAudioError, ApplicationAudioInputBridge,
    ApplicationDiscovery, ProcessInfo, TapStats, VirtualAudioInputStream,
};

// Re-export high-level audio manager
pub use manager::ApplicationAudioManager;

pub use file_player::{
    AudioFilePlayer, FilePlayerConfig, FilePlayerDevice, FilePlayerManager, FilePlayerService,
    PlaybackAction, PlaybackMode, PlaybackState, PlaybackStatus, QueuedTrack, RepeatMode,
};

pub use events::{MasterVULevelEvent, VUChannelData, VULevelEvent};
pub use vu_channel_service::VUChannelService;
