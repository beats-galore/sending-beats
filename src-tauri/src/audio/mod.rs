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