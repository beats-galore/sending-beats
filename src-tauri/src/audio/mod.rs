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
pub mod streams;
pub mod mixer;
pub mod database;
pub mod streaming_bridge;
pub mod device_monitor;

#[cfg(target_os = "macos")]
pub mod coreaudio_stream;

// Re-export commonly used types for easier imports
pub use types::{
    AudioChannel, AudioDeviceInfo, AudioMetrics, MixerCommand, MixerConfig,
    AudioConfigFactory, AudioDeviceHandle, 
};

#[cfg(target_os = "macos")]
pub use types::CoreAudioDevice;

pub use devices::AudioDeviceManager;

pub use effects::{
    AudioAnalyzer, AudioEffectsChain, EQBand, PeakDetector, RmsDetector,
    SpectrumAnalyzer, ThreeBandEqualizer, BiquadFilter, Compressor, Limiter,
};

pub use streams::{
    AudioInputStream, AudioOutputStream, VirtualMixerHandle, StreamCommand,
    StreamManager, get_stream_manager,
};

pub use mixer::VirtualMixer;

pub use database::{
    AudioDatabase, AudioEventBus, VULevelData, MasterLevelData, 
    AudioDeviceConfig, ChannelConfig, OutputRouteConfig,
};

pub use streaming_bridge::{
    AudioStreamingBridge, StreamingStatus, StreamingCommand, StreamingStats,
    create_streaming_bridge,
};

pub use device_monitor::{
    DeviceMonitor, DeviceMonitorConfig, DeviceMonitorStats,
    initialize_device_monitoring, get_device_monitor, stop_device_monitoring, get_device_monitoring_stats,
};

// Global audio debug logging control
pub static AUDIO_DEBUG_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[macro_export]
macro_rules! audio_debug {
    ($($arg:tt)*) => {
        if $crate::audio::AUDIO_DEBUG_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            println!($($arg)*);
        }
    };
}