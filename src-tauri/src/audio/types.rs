use serde::{Deserialize, Serialize};
use cpal::traits::DeviceTrait;

#[cfg(target_os = "macos")]
use coreaudio_sys::AudioDeviceID;
#[cfg(target_os = "macos")]
use crate::audio::devices::coreaudio_stream::CoreAudioOutputStream;

/// Audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub is_default: bool,
    pub supported_sample_rates: Vec<u32>,
    pub supported_channels: Vec<u16>,
    pub host_api: String,
}

/// Cross-platform audio device handle
pub enum AudioDeviceHandle {
    Cpal(cpal::Device),
    #[cfg(target_os = "macos")]
    CoreAudio(CoreAudioDevice),
}

impl std::fmt::Debug for AudioDeviceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioDeviceHandle::Cpal(device) => {
                f.debug_struct("AudioDeviceHandle::Cpal")
                    .field("name", &device.name().unwrap_or_else(|_| "Unknown".to_string()))
                    .finish()
            }
            #[cfg(target_os = "macos")]
            AudioDeviceHandle::CoreAudio(device) => {
                f.debug_struct("AudioDeviceHandle::CoreAudio")
                    .field("device", device)
                    .finish()
            }
        }
    }
}

/// CoreAudio specific device for direct hardware access
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct CoreAudioDevice {
    pub device_id: AudioDeviceID,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub stream: Option<CoreAudioOutputStream>,
}

/// Audio channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChannel {
    pub id: u32,
    pub name: String,
    pub input_device_id: Option<String>,
    pub gain: f32,         // Linear gain (0.0 - 2.0)
    pub pan: f32,          // Pan (-1.0 left to 1.0 right)
    pub muted: bool,
    pub solo: bool,
    pub effects_enabled: bool,
    pub peak_level: f32,   // Current peak level for VU meter
    pub rms_level: f32,    // RMS level for VU meter
    
    // EQ settings
    pub eq_low_gain: f32,    // Low band gain in dB (-12 to +12)
    pub eq_mid_gain: f32,    // Mid band gain in dB (-12 to +12)  
    pub eq_high_gain: f32,   // High band gain in dB (-12 to +12)
    
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
            gain: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
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
    pub gain: f32,         // Individual output gain (0.0 - 2.0)
    pub enabled: bool,     // Whether this output is active
    pub is_monitor: bool,  // Whether this is a monitor/headphone output
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
    pub output_devices: Vec<OutputDevice>, // New multiple output support
    pub enable_loopback: bool,
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 512,  // Ultra-low latency: ~10.7ms at 48kHz
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetrics {
    pub cpu_usage: f32,
    pub buffer_underruns: u64,
    pub buffer_overruns: u64,
    pub latency_ms: f32,
    pub sample_rate: u32,
    pub active_channels: u32,
}

/// Commands for controlling the mixer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MixerCommand {
    AddChannel(AudioChannel),
    RemoveChannel(u32),
    UpdateChannel(u32, AudioChannel),
    SetMasterGain(f32),
    StartStream,
    StopStream,
    EnableChannel(u32, bool),
    SoloChannel(u32, bool),
    MuteChannel(u32, bool),
    // Multiple output device commands
    AddOutputDevice(OutputDevice),
    RemoveOutputDevice(String), // device_id
    UpdateOutputDevice(String, OutputDevice), // device_id, new_config
    SetOutputDeviceGain(String, f32), // device_id, gain
    EnableOutputDevice(String, bool), // device_id, enabled
}

/// Factory for creating optimized audio configurations based on use case
pub struct AudioConfigFactory;

impl AudioConfigFactory {
    /// Create configuration optimized for ultra-low latency DJing
    pub fn create_dj_config() -> MixerConfig {
        MixerConfig {
            sample_rate: 48000,
            buffer_size: 256,  // ~5.3ms latency at 48kHz
            channels: vec![
                AudioChannel { id: 1, name: "Deck A".to_string(), ..Default::default() },
                AudioChannel { id: 2, name: "Deck B".to_string(), ..Default::default() },
                AudioChannel { id: 3, name: "Microphone".to_string(), ..Default::default() },
                AudioChannel { id: 4, name: "System Audio".to_string(), ..Default::default() },
            ],
            master_gain: 1.0,
            enable_loopback: true,
            ..Default::default()
        }
    }

    /// Create configuration optimized for streaming/recording
    pub fn create_streaming_config() -> MixerConfig {
        MixerConfig {
            sample_rate: 48000,
            buffer_size: 1024, // ~21.3ms latency - acceptable for streaming
            channels: vec![
                AudioChannel { id: 1, name: "Microphone".to_string(), ..Default::default() },
                AudioChannel { id: 2, name: "Desktop Audio".to_string(), ..Default::default() },
                AudioChannel { id: 3, name: "Music".to_string(), ..Default::default() },
                AudioChannel { id: 4, name: "Game Audio".to_string(), ..Default::default() },
            ],
            master_gain: 1.0,
            enable_loopback: true,
            ..Default::default()
        }
    }
}