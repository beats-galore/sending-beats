// Audio module - Modularized audio system for Sending Beats
//
// This module provides a comprehensive audio processing system broken down into logical components:
// - types: Core data types and configurations
// - devices: Audio device management and enumeration
// - effects: Audio effects processing (EQ, compression, limiting, analysis)
// - streams: Audio stream management (input/output)  
// - mixer: Core virtual mixer functionality

pub mod types;
pub mod devices;
pub mod effects;
pub mod mixer;
pub mod broadcasting;
pub mod recording;
pub mod tap;
pub mod file_player;
pub mod manager;


// Re-export commonly used types for easier imports
pub use types::{
    AudioChannel, AudioDeviceInfo, AudioMetrics, MixerCommand, MixerConfig,
    AudioConfigFactory, AudioDeviceHandle, 
};

#[cfg(target_os = "macos")]
pub use types::CoreAudioDevice;


pub use effects::{
    AudioAnalyzer, AudioEffectsChain, EQBand, PeakDetector, RmsDetector,
    SpectrumAnalyzer, ThreeBandEqualizer, BiquadFilter, Compressor, Limiter,
};

pub use mixer::{
    VirtualMixer,
    AudioInputStream, AudioOutputStream, VirtualMixerHandle, StreamCommand,
    get_stream_manager,
};

pub use crate::db::{
    AudioDatabase, AudioEventBus, VULevelData, MasterLevelData, 
    AudioDeviceConfig, ChannelConfig, OutputRouteConfig,
};

pub use broadcasting::{
    AudioStreamingBridge, StreamingStatus, StreamingCommand, StreamingStats,
    create_streaming_bridge, StreamManager, StreamConfig, StreamingService, 
    AudioEncoder, IcecastSourceClient, IcecastStats, IcecastStreamManager,
};

pub use devices::{
    AudioDeviceManager, DeviceMonitor, DeviceMonitorConfig, DeviceMonitorStats,
    initialize_device_monitoring, get_device_monitor, stop_device_monitoring, get_device_monitoring_stats,
};

pub use recording::{
    RecordingService, RecordingConfig, RecordingStatus, RecordingHistoryEntry,
    RecordingFormat, RecordingMetadata, RecordingSession, RecordingWriter, RecordingCommand,
};

pub use tap::{
    ProcessInfo, TapStats, ApplicationAudioError,
    ApplicationDiscovery, VirtualAudioInputStream, ApplicationAudioInputBridge,
    get_virtual_input_registry,
};

// Re-export high-level audio manager
pub use manager::ApplicationAudioManager;

pub use file_player::{
    AudioFilePlayer, FilePlayerDevice, QueuedTrack, PlaybackState, PlaybackStatus, PlaybackMode, RepeatMode,
    FilePlayerManager, FilePlayerService, FilePlayerConfig, PlaybackAction,
};

