use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use crate::audio::devices::coreaudio_stream::CoreAudioOutputStream;
#[cfg(target_os = "macos")]
use coreaudio_sys::AudioDeviceID;

/// Audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    pub is_input: bool,
    pub is_output: bool,
    pub is_default: bool,
    pub supported_sample_rates: Vec<u32>,
    pub supported_channels: Vec<u16>,
    pub host_api: String,
}

/// Cross-platform audio device handle
pub enum AudioDeviceHandle {
    #[cfg(target_os = "macos")]
    CoreAudio(CoreAudioDevice),
    #[cfg(target_os = "macos")]
    ApplicationAudio(ApplicationAudioDevice),
}

impl std::fmt::Debug for AudioDeviceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_os = "macos")]
            AudioDeviceHandle::CoreAudio(device) => f
                .debug_struct("AudioDeviceHandle::CoreAudio")
                .field("device", device)
                .finish(),
            #[cfg(target_os = "macos")]
            AudioDeviceHandle::ApplicationAudio(device) => f
                .debug_struct("AudioDeviceHandle::ApplicationAudio")
                .field("device", device)
                .finish(),
        }
    }
}

/// CoreAudio specific device for direct hardware access
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct CoreAudioDevice {
    pub device_id: AudioDeviceID,
    pub name: String,
    pub uid: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub stream: Option<CoreAudioOutputStream>,
}

/// Application audio device for capturing audio from specific applications
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct ApplicationAudioDevice {
    pub pid: u32,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Audio channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChannel {
    pub id: u32,
    pub name: String,
    pub input_device_id: Option<String>,
    pub effects_enabled: bool,
    pub peak_level: f32, // Current peak level for VU meter
    pub rms_level: f32,  // RMS level for VU meter

    // EQ settings
    pub eq_low_gain: f32,  // Low band gain in dB (-12 to +12)
    pub eq_mid_gain: f32,  // Mid band gain in dB (-12 to +12)
    pub eq_high_gain: f32, // High band gain in dB (-12 to +12)

    // Compressor settings
    pub comp_threshold: f32, // Threshold in dB (-40 to 0)
    pub comp_ratio: f32,     // Compression ratio (1.0 to 10.0)
    pub comp_attack: f32,    // Attack time in ms (0.1 to 100)
    pub comp_release: f32,   // Release time in ms (10 to 1000)
    pub comp_enabled: bool,

    // Limiter settings
    pub limiter_threshold: f32, // Limiter threshold in dB (-12 to 0)
    pub limiter_enabled: bool,
}

impl Default for AudioChannel {
    fn default() -> Self {
        Self {
            id: 0,
            name: "Channel".to_string(),
            input_device_id: None,
            effects_enabled: false,
            peak_level: 0.0,
            rms_level: 0.0,

            // EQ defaults (flat response)
            eq_low_gain: 0.0,
            eq_mid_gain: 0.0,
            eq_high_gain: 0.0,

            // Compressor defaults
            comp_threshold: -12.0,
            comp_ratio: 4.0,
            comp_attack: 5.0,
            comp_release: 100.0,
            comp_enabled: false,

            // Limiter defaults
            limiter_threshold: -0.1,
            limiter_enabled: false,
        }
    }
}

/// Output device configuration for multiple output support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDevice {
    pub device_id: String,
    pub device_name: String,
    pub gain: f32,        // Individual output gain (0.0 - 2.0)
    pub enabled: bool,    // Whether this output is active
    pub is_monitor: bool, // Whether this is a monitor/headphone output
}

/// Virtual mixer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub channels: Vec<AudioChannel>,
    pub master_gain: f32,
    pub master_output_device_id: Option<String>, // Kept for backward compatibility
    pub monitor_output_device_id: Option<String>, // Kept for backward compatibility
    pub output_devices: Vec<OutputDevice>,       // New multiple output support
    pub enable_loopback: bool,
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self {
            sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
            buffer_size: 512, // Ultra-low latency: ~10.7ms at 48kHz
            channels: vec![],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            output_devices: vec![], // Empty by default
            enable_loopback: true,
        }
    }
}

/// Audio metrics for monitoring
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioMetrics {
    pub cpu_usage: f32,
    pub buffer_underruns: u64,
    pub buffer_overruns: u64,
    pub latency_ms: f32,
    pub sample_rate: u32,
    pub active_channels: u32,
    pub samples_processed: u64,
    #[serde(skip)]
    pub last_process_time: std::time::Instant,
}

impl Default for AudioMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            latency_ms: 0.0,
            sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
            active_channels: 0,
            samples_processed: 0,
            last_process_time: std::time::Instant::now(),
        }
    }
}

/// Factory for creating optimized audio configurations based on use case
pub struct AudioConfigFactory;

impl AudioConfigFactory {
    /// Create configuration optimized for ultra-low latency DJing
    pub fn create_dj_config() -> MixerConfig {
        MixerConfig {
            sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
            buffer_size: 256, // ~5.3ms latency at 48kHz
            channels: vec![
                AudioChannel {
                    id: 0,
                    name: "Deck A".to_string(),
                    ..Default::default()
                },
                AudioChannel {
                    id: 1,
                    name: "Deck B".to_string(),
                    ..Default::default()
                },
                AudioChannel {
                    id: 2,
                    name: "Microphone".to_string(),
                    ..Default::default()
                },
                AudioChannel {
                    id: 3,
                    name: "System Audio".to_string(),
                    ..Default::default()
                },
            ],
            master_gain: 1.0,
            enable_loopback: true,
            ..Default::default()
        }
    }

    /// Create configuration optimized for streaming/recording
    pub fn create_streaming_config() -> MixerConfig {
        MixerConfig {
            sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
            buffer_size: 1024, // ~21.3ms latency - acceptable for streaming
            channels: vec![
                AudioChannel {
                    id: 1,
                    name: "Microphone".to_string(),
                    ..Default::default()
                },
                AudioChannel {
                    id: 2,
                    name: "Desktop Audio".to_string(),
                    ..Default::default()
                },
                AudioChannel {
                    id: 3,
                    name: "Music".to_string(),
                    ..Default::default()
                },
                AudioChannel {
                    id: 4,
                    name: "Game Audio".to_string(),
                    ..Default::default()
                },
            ],
            master_gain: 1.0,
            enable_loopback: true,
            ..Default::default()
        }
    }
}
