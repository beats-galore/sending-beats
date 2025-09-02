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
pub mod file_player;
pub mod manager;
pub mod mixer;
pub mod recording;
pub mod tap;
pub mod types;

// Re-export commonly used types for easier imports
pub use types::{
    AudioChannel, AudioConfigFactory, AudioDeviceHandle, AudioDeviceInfo, AudioMetrics,
    MixerCommand, MixerConfig,
};

#[cfg(target_os = "macos")]
pub use types::CoreAudioDevice;

pub use effects::{
    AudioAnalyzer, AudioEffectsChain, BiquadFilter, Compressor, EQBand, Limiter, PeakDetector,
    RmsDetector, SpectrumAnalyzer, ThreeBandEqualizer,
};

pub use mixer::{
    get_stream_manager, AudioInputStream, AudioOutputStream, StreamCommand, VirtualMixer,
    VirtualMixerHandle,
};

pub use crate::db::{
    AudioDatabase, AudioDeviceConfig, AudioEventBus, ChannelConfig, MasterLevelData,
    OutputRouteConfig, VULevelData,
};

pub use broadcasting::{
    create_streaming_bridge, AudioEncoder, AudioStreamingBridge, IcecastSourceClient, IcecastStats,
    IcecastStreamManager, StreamConfig, StreamManager, StreamingCommand, StreamingService,
    StreamingStats, StreamingStatus,
};

pub use devices::{
    get_device_monitor, get_device_monitoring_stats, initialize_device_monitoring,
    stop_device_monitoring, AudioDeviceManager, DeviceMonitor, DeviceMonitorConfig,
    DeviceMonitorStats,
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
