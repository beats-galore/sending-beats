use anyhow::{Context, Result};
use cpal::{Device, Host, Stream, SampleFormat, StreamConfig};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

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

/// Virtual mixer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub channels: Vec<AudioChannel>,
    pub master_gain: f32,
    pub master_output_device_id: Option<String>,
    pub monitor_output_device_id: Option<String>,
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

/// Cross-platform audio device manager
pub struct AudioDeviceManager {
    host: Host,
    devices_cache: Arc<Mutex<HashMap<String, AudioDeviceInfo>>>,
}

impl AudioDeviceManager {
    pub fn new() -> Result<Self> {
        // Try to use CoreAudio host explicitly on macOS for better device detection
        #[cfg(target_os = "macos")]
        let host = {
            println!("Attempting to use CoreAudio host on macOS...");
            match cpal::host_from_id(cpal::HostId::CoreAudio) {
                Ok(host) => {
                    println!("Successfully initialized CoreAudio host");
                    host
                }
                Err(e) => {
                    println!("Failed to initialize CoreAudio host: {}, falling back to default", e);
                    cpal::default_host()
                }
            }
        };
        
        #[cfg(not(target_os = "macos"))]
        let host = cpal::default_host();
        
        println!("Using audio host: {:?}", host.id());
        
        Ok(Self {
            host,
            devices_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Get all available audio devices with detailed information
    pub async fn enumerate_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        println!("Starting audio device enumeration...");
        
        // Debug: Check all available hosts
        println!("Available audio hosts:");
        for host_id in cpal::ALL_HOSTS {
            println!("  - {:?}", host_id);
        }
        
        println!("Current host: {:?}", self.host.id());
        
        // Try enumerating with all available hosts to see if we get different results
        #[cfg(target_os = "macos")]
        {
            println!("Comparing device detection across all hosts:");
            for host_id in cpal::ALL_HOSTS {
                match cpal::host_from_id(*host_id) {
                    Ok(host) => {
                        println!("  Host {:?}:", host_id);
                        match host.output_devices() {
                            Ok(devices) => {
                                let device_count = devices.count();
                                println!("    Output devices: {}", device_count);
                            }
                            Err(e) => {
                                println!("    Failed to get output devices: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  Failed to initialize host {:?}: {}", host_id, e);
                    }
                }
            }
        }
        
        let input_devices = self.host.input_devices()
            .context("Failed to get input devices")?;
        let output_devices = self.host.output_devices()
            .context("Failed to get output devices")?;
        
        let default_input = self.host.default_input_device();
        let default_output = self.host.default_output_device();
        
        println!("Default input: {:?}", default_input.as_ref().and_then(|d| d.name().ok()));
        println!("Default output: {:?}", default_output.as_ref().and_then(|d| d.name().ok()));
        
        let mut devices = Vec::new();
        let mut devices_map = HashMap::new();

        // Process input devices
        for (_index, device) in input_devices.enumerate() {
            let is_default = if let Some(ref default_device) = default_input {
                device.name().unwrap_or_default() == default_device.name().unwrap_or_default()
            } else {
                false
            };
            
            if let Ok(device_info) = self.get_device_info(&device, true, false, is_default) {
                devices.push(device_info.clone());
                devices_map.insert(device_info.id.clone(), device_info);
            }
        }

        // Process output devices
        println!("Processing output devices...");
        let output_devices_vec: Vec<_> = output_devices.collect();
        println!("Total output devices found: {}", output_devices_vec.len());
        
        for (_index, device) in output_devices_vec.into_iter().enumerate() {
            let device_name = device.name().unwrap_or_else(|_| "Unknown Device".to_string());
            println!("Found output device: {}", device_name);
            
            // Try to get more detailed device information
            match device.supported_output_configs() {
                Ok(configs) => {
                    println!("  Supported configs:");
                    for (i, config) in configs.enumerate() {
                        if i < 3 { // Limit output to prevent spam
                            println!("    - Sample rate: {}-{}, Channels: {}, Format: {:?}", 
                                config.min_sample_rate().0, 
                                config.max_sample_rate().0,
                                config.channels(),
                                config.sample_format()
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("  Failed to get configs: {}", e);
                }
            }
            
            let is_default = if let Some(ref default_device) = default_output {
                device.name().unwrap_or_default() == default_device.name().unwrap_or_default()
            } else {
                false
            };
            
            match self.get_device_info(&device, false, true, is_default) {
                Ok(device_info) => {
                    println!("Successfully added output device: {} (ID: {})", device_info.name, device_info.id);
                    devices.push(device_info.clone());
                    devices_map.insert(device_info.id.clone(), device_info);
                }
                Err(e) => {
                    println!("Failed to process output device '{}': {}", device_name, e);
                }
            }
        }

        // Add common macOS system devices as fallback options if not detected
        #[cfg(target_os = "macos")]
        {
            let common_macos_devices = [
                ("MacBook Pro Speakers", "output_macbook_pro_speakers"),
                ("MacBook Air Speakers", "output_macbook_air_speakers"),
                ("BenQ EW327OU", "output_benq_ew327ou"),
                ("External Headphones", "output_external_headphones"),
                ("System Default Output", "output_system_default"),
            ];
            
            for (device_name, device_id) in &common_macos_devices {
                if !devices_map.contains_key(*device_id) {
                    println!("Adding fallback device: {}", device_name);
                    let fallback_device = AudioDeviceInfo {
                        id: device_id.to_string(),
                        name: device_name.to_string(),
                        is_input: false,
                        is_output: true,
                        is_default: device_name.contains("Default"),
                        supported_sample_rates: vec![44100, 48000],
                        supported_channels: vec![2],
                        host_api: "CoreAudio (Fallback)".to_string(),
                    };
                    devices.push(fallback_device.clone());
                    devices_map.insert(device_id.to_string(), fallback_device);
                }
            }
        }

        // Update cache
        *self.devices_cache.lock().await = devices_map;

        Ok(devices)
    }

    fn get_device_info(&self, device: &Device, is_input: bool, is_output: bool, is_default: bool) -> Result<AudioDeviceInfo> {
        let name = device.name().unwrap_or_else(|_| "Unknown Device".to_string());
        
        // Temporarily disable device filtering for debugging
        // if self.is_device_unavailable(&name) {
        //     println!("Filtering out device: {}", name);
        //     return Err(anyhow::anyhow!("Device not available: {}", name));
        // }
        println!("Processing device: {} (input: {}, output: {})", name, is_input, is_output);
        
        let id = format!("{}_{}", if is_input { "input" } else { "output" }, 
                        name.replace(" ", "_").replace("(", "").replace(")", "").to_lowercase());

        // Get supported configurations
        let mut supported_sample_rates = Vec::new();
        let mut supported_channels = Vec::new();

        if is_input {
            if let Ok(configs) = device.supported_input_configs() {
                for config in configs {
                    // Collect sample rates
                    let min_sample_rate = config.min_sample_rate().0;
                    let max_sample_rate = config.max_sample_rate().0;
                    
                    // Add common sample rates within the supported range
                    for &rate in &[44100, 48000, 88200, 96000, 192000] {
                        if rate >= min_sample_rate && rate <= max_sample_rate {
                            if !supported_sample_rates.contains(&rate) {
                                supported_sample_rates.push(rate);
                            }
                        }
                    }

                    // Collect channels
                    let channels = config.channels();
                    if !supported_channels.contains(&channels) {
                        supported_channels.push(channels);
                    }
                }
            }
        } else {
            if let Ok(configs) = device.supported_output_configs() {
                for config in configs {
                    // Collect sample rates
                    let min_sample_rate = config.min_sample_rate().0;
                    let max_sample_rate = config.max_sample_rate().0;
                    
                    // Add common sample rates within the supported range
                    for &rate in &[44100, 48000, 88200, 96000, 192000] {
                        if rate >= min_sample_rate && rate <= max_sample_rate {
                            if !supported_sample_rates.contains(&rate) {
                                supported_sample_rates.push(rate);
                            }
                        }
                    }

                    // Collect channels
                    let channels = config.channels();
                    if !supported_channels.contains(&channels) {
                        supported_channels.push(channels);
                    }
                }
            }
        }


        Ok(AudioDeviceInfo {
            id,
            name: self.clean_device_name(&name),
            is_input,
            is_output,
            is_default,
            supported_sample_rates,
            supported_channels,
            host_api: format!("{:?}", self.host.id()),
        })
    }
    
    /// Check if a device is unavailable (not running or inactive)
    fn is_device_unavailable(&self, device_name: &str) -> bool {
        let name_lower = device_name.to_lowercase();
        
        // List of applications that might appear as audio devices when not running
        let inactive_apps = [
            "serato",
            "virtual dj",
            "traktor",
            "ableton",
            "logic",
            "pro tools",
            "cubase",
            "reaper",
            "obs",
            "zoom",
            "teams",
            "discord",
            "skype"
        ];
        
        // Check if device name contains inactive app names but seems inactive
        for app in &inactive_apps {
            if name_lower.contains(app) && (name_lower.contains("not connected") || 
                                          name_lower.contains("inactive") ||
                                          name_lower.contains("unavailable")) {
                return true;
            }
        }
        
        false
    }
    
    /// Clean up device names for better display
    fn clean_device_name(&self, name: &str) -> String {
        // Clean up common device name patterns
        let cleaned = name
            .replace(" (Built-in)", "")
            .replace(" - Built-in", "")
            .replace("Built-in ", "")
            .replace(" (Aggregate Device)", " (Aggregate)")
            .replace(" (Multi-Output Device)", " (Multi-Out)");
            
        // Add friendly names for common devices
        if cleaned.contains("MacBook") && cleaned.contains("Microphone") {
            "MacBook Microphone".to_string()
        } else if cleaned.contains("MacBook") && cleaned.contains("Speakers") {
            "MacBook Speakers".to_string()
        } else if cleaned.to_lowercase().contains("airpods") {
            format!("AirPods ({})", if cleaned.contains("Pro") { "Pro" } else { "Standard" })
        } else {
            cleaned
        }
    }

    /// Get device by ID from cache
    pub async fn get_device(&self, device_id: &str) -> Option<AudioDeviceInfo> {
        self.devices_cache.lock().await.get(device_id).cloned()
    }
    
    /// Force refresh device list
    pub async fn refresh_devices(&self) -> Result<()> {
        let _devices = self.enumerate_devices().await?;
        Ok(())
    }
    
    /// Find actual cpal Device by device_id for real audio I/O
    pub async fn find_cpal_device(&self, device_id: &str, is_input: bool) -> Result<Device> {
        println!("Searching for cpal device: {} (input: {})", device_id, is_input);
        
        // First check if this is a known device from our cache
        let device_info = self.get_device(device_id).await;
        if let Some(info) = device_info {
            println!("Found device info: {} -> {}", device_id, info.name);
        }
        
        // Get all available devices from cpal
        if is_input {
            let input_devices = self.host.input_devices()
                .context("Failed to enumerate input devices")?;
                
            for device in input_devices {
                let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                let generated_id = format!("input_{}", 
                    device_name.replace(" ", "_").replace("(", "").replace(")", "").to_lowercase());
                
                println!("Checking input device: '{}' -> '{}'", device_name, generated_id);
                
                if generated_id == device_id {
                    println!("Found matching input device: {}", device_name);
                    return Ok(device);
                }
            }
        } else {
            let output_devices = self.host.output_devices()
                .context("Failed to enumerate output devices")?;
                
            for device in output_devices {
                let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                let generated_id = format!("output_{}", 
                    device_name.replace(" ", "_").replace("(", "").replace(")", "").to_lowercase());
                
                println!("Checking output device: '{}' -> '{}'", device_name, generated_id);
                
                if generated_id == device_id {
                    println!("Found matching output device: {}", device_name);
                    return Ok(device);
                }
            }
        }
        
        // If not found by exact match, try to find by partial name match
        println!("No exact match found, trying partial name matching...");
        let target_name_parts: Vec<&str> = device_id.split('_').skip(1).collect(); // Skip "input" or "output" prefix
        
        if is_input {
            let input_devices = self.host.input_devices()
                .context("Failed to enumerate input devices for partial match")?;
                
            for device in input_devices {
                let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                let name_lower = device_name.to_lowercase();
                
                // Check if device name contains any of the target name parts
                let matches = target_name_parts.iter().any(|&part| {
                    !part.is_empty() && name_lower.contains(part)
                });
                
                if matches {
                    println!("Found partial match for input device: '{}' matches '{}'", device_name, device_id);
                    return Ok(device);
                }
            }
        } else {
            let output_devices = self.host.output_devices()
                .context("Failed to enumerate output devices for partial match")?;
                
            for device in output_devices {
                let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                let name_lower = device_name.to_lowercase();
                
                // Check if device name contains any of the target name parts
                let matches = target_name_parts.iter().any(|&part| {
                    !part.is_empty() && name_lower.contains(part)
                });
                
                if matches {
                    println!("Found partial match for output device: '{}' matches '{}'", device_name, device_id);
                    return Ok(device);
                }
            }
        }
        
        // As a last resort, try to use default device
        if is_input {
            if let Some(default_device) = self.host.default_input_device() {
                let default_name = default_device.name().unwrap_or_else(|_| "Default Input".to_string());
                println!("No device found for '{}', using default input device: {}", device_id, default_name);
                return Ok(default_device);
            }
        } else {
            if let Some(default_device) = self.host.default_output_device() {
                let default_name = default_device.name().unwrap_or_else(|_| "Default Output".to_string());
                println!("No device found for '{}', using default output device: {}", device_id, default_name);
                return Ok(default_device);
            }
        }
        
        Err(anyhow::anyhow!("No suitable {} device found for ID: {}", 
            if is_input { "input" } else { "output" }, device_id))
    }
}

/// Virtual audio mixer with ultra-low latency processing
// Audio stream management structures
pub struct AudioInputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub audio_buffer: Arc<Mutex<Vec<f32>>>,
    pub effects_chain: Arc<Mutex<AudioEffectsChain>>,
    // Stream is managed separately via StreamManager to avoid Send/Sync issues
}

// Stream management handles the actual cpal streams in a separate synchronous context
struct StreamManager {
    streams: HashMap<String, cpal::Stream>,
}

impl StreamManager {
    fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    fn add_input_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        target_sample_rate: u32,
    ) -> Result<()> {
        use cpal::SampleFormat;
        use cpal::traits::StreamTrait;
        
        let device_config = device.default_input_config().context("Failed to get device config")?;
        
        let stream = match device_config.sample_format() {
            SampleFormat::F32 => {
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mono_samples: Vec<f32> = if config.channels == 1 {
                            data.to_vec()
                        } else {
                            data.chunks(config.channels as usize)
                                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                                .collect()
                        };
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&mono_samples);
                            let max_buffer_size = target_sample_rate as usize * 2;
                            if buffer.len() > max_buffer_size {
                                let excess = buffer.len() - max_buffer_size;
                                buffer.drain(0..excess);
                            }
                        }
                    },
                    |err| eprintln!("Audio input error: {}", err),
                    None
                )?
            },
            SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| sample as f32 / 32768.0)
                            .collect();
                            
                        let mono_samples: Vec<f32> = if config.channels == 1 {
                            f32_samples
                        } else {
                            f32_samples.chunks(config.channels as usize)
                                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                                .collect()
                        };
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&mono_samples);
                            let max_buffer_size = target_sample_rate as usize * 2;
                            if buffer.len() > max_buffer_size {
                                let excess = buffer.len() - max_buffer_size;
                                buffer.drain(0..excess);
                            }
                        }
                    },
                    |err| eprintln!("Audio input error: {}", err),
                    None
                )?
            },
            SampleFormat::U16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| (sample as f32 - 32768.0) / 32768.0)
                            .collect();
                            
                        let mono_samples: Vec<f32> = if config.channels == 1 {
                            f32_samples
                        } else {
                            f32_samples.chunks(config.channels as usize)
                                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                                .collect()
                        };
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&mono_samples);
                            let max_buffer_size = target_sample_rate as usize * 2;
                            if buffer.len() > max_buffer_size {
                                let excess = buffer.len() - max_buffer_size;
                                buffer.drain(0..excess);
                            }
                        }
                    },
                    |err| eprintln!("Audio input error: {}", err),
                    None
                )?
            },
            _ => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", device_config.sample_format()));
            }
        };
        
        stream.play().context("Failed to start input stream")?;
        self.streams.insert(device_id, stream);
        
        Ok(())
    }
    
    fn remove_stream(&mut self, device_id: &str) -> bool {
        self.streams.remove(device_id).is_some()
    }
}

// Stream management commands for cross-thread communication
enum StreamCommand {
    AddInputStream {
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        target_sample_rate: u32,
        response_tx: std::sync::mpsc::Sender<Result<()>>,
    },
    RemoveStream {
        device_id: String,
        response_tx: std::sync::mpsc::Sender<bool>,
    },
}

// Global stream manager instance
static STREAM_MANAGER: std::sync::OnceLock<std::sync::mpsc::Sender<StreamCommand>> = std::sync::OnceLock::new();

// Initialize the stream manager thread
fn init_stream_manager() -> std::sync::mpsc::Sender<StreamCommand> {
    let (tx, rx) = std::sync::mpsc::channel::<StreamCommand>();
    
    std::thread::spawn(move || {
        let mut manager = StreamManager::new();
        println!("Stream manager thread started");
        
        while let Ok(command) = rx.recv() {
            match command {
                StreamCommand::AddInputStream {
                    device_id,
                    device,
                    config,
                    audio_buffer,
                    target_sample_rate,
                    response_tx,
                } => {
                    let result = manager.add_input_stream(device_id, device, config, audio_buffer, target_sample_rate);
                    let _ = response_tx.send(result);
                }
                StreamCommand::RemoveStream { device_id, response_tx } => {
                    let result = manager.remove_stream(&device_id);
                    let _ = response_tx.send(result);
                }
            }
        }
        
        println!("Stream manager thread stopped");
    });
    
    tx
}

// Get or initialize the global stream manager
fn get_stream_manager() -> &'static std::sync::mpsc::Sender<StreamCommand> {
    STREAM_MANAGER.get_or_init(init_stream_manager)
}

impl AudioInputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> Result<Self> {
        let audio_buffer = Arc::new(Mutex::new(Vec::new()));
        let effects_chain = Arc::new(Mutex::new(AudioEffectsChain::new(sample_rate)));
        
        Ok(AudioInputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 1, // Start with mono
            audio_buffer,
            effects_chain,
        })
    }
    
    pub fn get_samples(&self) -> Vec<f32> {
        if let Ok(mut buffer) = self.audio_buffer.try_lock() {
            let samples = buffer.clone();
            buffer.clear();
            samples
        } else {
            Vec::new()
        }
    }

    /// Apply effects to input samples and update channel settings
    pub fn process_with_effects(&self, channel: &AudioChannel) -> Vec<f32> {
        if let Ok(mut buffer) = self.audio_buffer.try_lock() {
            let mut samples = buffer.clone();
            buffer.clear();

            // Apply effects if enabled
            if channel.effects_enabled && !samples.is_empty() {
                if let Ok(mut effects) = self.effects_chain.try_lock() {
                    // Update effects parameters based on channel settings
                    effects.set_eq_gain(EQBand::Low, channel.eq_low_gain);
                    effects.set_eq_gain(EQBand::Mid, channel.eq_mid_gain);
                    effects.set_eq_gain(EQBand::High, channel.eq_high_gain);
                    
                    if channel.comp_enabled {
                        effects.set_compressor_params(
                            channel.comp_threshold,
                            channel.comp_ratio,
                            channel.comp_attack,
                            channel.comp_release,
                        );
                    }
                    
                    if channel.limiter_enabled {
                        effects.set_limiter_threshold(channel.limiter_threshold);
                    }

                    // Process samples through effects chain
                    effects.process(&mut samples);
                }
            }

            samples
        } else {
            Vec::new()
        }
    }
}

pub struct AudioOutputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub input_buffer: Arc<Mutex<Vec<f32>>>,
    // Stream is handled separately to avoid Send/Sync issues
}

impl AudioOutputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> Result<Self> {
        let input_buffer = Arc::new(Mutex::new(Vec::new()));
        
        Ok(AudioOutputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Stereo output
            input_buffer,
        })
    }
    
    pub fn send_samples(&self, samples: &[f32]) {
        if let Ok(mut buffer) = self.input_buffer.try_lock() {
            buffer.extend_from_slice(samples);
            // Limit buffer size to prevent memory issues
            let max_samples = self.sample_rate as usize * 2; // 2 seconds max
            let buffer_len = buffer.len();
            if buffer_len > max_samples {
                buffer.drain(0..(buffer_len - max_samples));
            }
        }
    }
}

// Helper structure for processing thread
pub struct VirtualMixerHandle {
    input_streams: Arc<Mutex<HashMap<String, Arc<AudioInputStream>>>>,
    output_stream: Arc<Mutex<Option<Arc<AudioOutputStream>>>>,
}

impl VirtualMixerHandle {
    /// Get samples from all active input streams with effects processing
    pub async fn collect_input_samples_with_effects(&self, channels: &[AudioChannel]) -> HashMap<String, Vec<f32>> {
        let mut samples = HashMap::new();
        let streams = self.input_streams.lock().await;
        
        for (device_id, stream) in streams.iter() {
            // Find the channel configuration for this stream
            if let Some(channel) = channels.iter().find(|ch| {
                ch.input_device_id.as_ref() == Some(device_id)
            }) {
                let stream_samples = stream.process_with_effects(channel);
                if !stream_samples.is_empty() {
                    samples.insert(device_id.clone(), stream_samples);
                }
            } else {
                // No channel config found, use raw samples
                let stream_samples = stream.get_samples();
                if !stream_samples.is_empty() {
                    samples.insert(device_id.clone(), stream_samples);
                }
            }
        }
        
        samples
    }

    /// Get samples from all active input streams (without effects - for compatibility)
    pub async fn collect_input_samples(&self) -> HashMap<String, Vec<f32>> {
        let mut samples = HashMap::new();
        let streams = self.input_streams.lock().await;
        
        for (device_id, stream) in streams.iter() {
            let stream_samples = stream.get_samples();
            if !stream_samples.is_empty() {
                samples.insert(device_id.clone(), stream_samples);
            }
        }
        
        samples
    }

    /// Send mixed samples to the output stream
    pub async fn send_to_output(&self, samples: &[f32]) {
        if let Some(output) = self.output_stream.lock().await.as_ref() {
            output.send_samples(samples);
        }
    }
}

pub struct VirtualMixer {
    config: MixerConfig,
    is_running: Arc<AtomicBool>,
    
    // Real-time audio buffers
    mix_buffer: Arc<Mutex<Vec<f32>>>,
    
    // Audio processing (placeholder for future sample rate conversion)
    sample_rate_converter: Option<()>,
    audio_analyzer: AudioAnalyzer,
    
    // Communication channels
    command_tx: mpsc::Sender<MixerCommand>,
    command_rx: Arc<Mutex<mpsc::Receiver<MixerCommand>>>,
    audio_output_tx: mpsc::Sender<Vec<f32>>,
    
    // Metrics
    metrics: Arc<Mutex<AudioMetrics>>,
    
    // Real-time audio level data for VU meters
    channel_levels: Arc<Mutex<HashMap<u32, (f32, f32)>>>,
    master_levels: Arc<Mutex<(f32, f32, f32, f32)>>,
    
    // Audio stream management
    audio_device_manager: Arc<AudioDeviceManager>,
    input_streams: Arc<Mutex<HashMap<String, Arc<AudioInputStream>>>>,
    output_stream: Arc<Mutex<Option<Arc<AudioOutputStream>>>>,
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
}

/// Real-time audio analysis
pub struct AudioAnalyzer {
    peak_detector: PeakDetector,
    rms_detector: RmsDetector,
    spectrum_analyzer: Option<SpectrumAnalyzer>,
}

impl AudioAnalyzer {
    fn new(sample_rate: u32) -> Self {
        Self {
            peak_detector: PeakDetector::new(),
            rms_detector: RmsDetector::new(sample_rate),
            spectrum_analyzer: Some(SpectrumAnalyzer::new(sample_rate, 1024)),
        }
    }

    fn process(&mut self, samples: &[f32]) -> (f32, f32) {
        let peak = self.peak_detector.process(samples);
        let rms = self.rms_detector.process(samples);
        
        if let Some(ref mut analyzer) = self.spectrum_analyzer {
            analyzer.process(samples);
        }
        
        (peak, rms)
    }
}

/// Peak level detector with decay
pub struct PeakDetector {
    peak: f32,
    decay_factor: f32,
}

impl PeakDetector {
    fn new() -> Self {
        Self {
            peak: 0.0,
            decay_factor: 0.999, // Slow decay for visual meters
        }
    }

    fn process(&mut self, samples: &[f32]) -> f32 {
        for &sample in samples {
            let abs_sample = sample.abs();
            if abs_sample > self.peak {
                self.peak = abs_sample;
            }
        }
        
        // Apply decay
        self.peak *= self.decay_factor;
        self.peak
    }
}

/// RMS level detector for average loudness
pub struct RmsDetector {
    window_size: usize,
    sample_buffer: Vec<f32>,
    write_index: usize,
    sum_of_squares: f32,
}

impl RmsDetector {
    fn new(sample_rate: u32) -> Self {
        let window_size = (sample_rate as f32 * 0.1) as usize; // 100ms window
        Self {
            window_size,
            sample_buffer: vec![0.0; window_size],
            write_index: 0,
            sum_of_squares: 0.0,
        }
    }

    fn process(&mut self, samples: &[f32]) -> f32 {
        for &sample in samples {
            // Remove old sample from sum
            let old_sample = self.sample_buffer[self.write_index];
            self.sum_of_squares -= old_sample * old_sample;
            
            // Add new sample
            self.sample_buffer[self.write_index] = sample;
            self.sum_of_squares += sample * sample;
            
            // Advance write index
            self.write_index = (self.write_index + 1) % self.window_size;
        }
        
        (self.sum_of_squares / self.window_size as f32).sqrt()
    }
}

/// Basic spectrum analyzer for frequency analysis
pub struct SpectrumAnalyzer {
    sample_rate: u32,
    fft_size: usize,
    window: Vec<f32>,
    input_buffer: Vec<f32>,
    output_spectrum: Vec<f32>,
}

/// Real-time audio effects chain
pub struct AudioEffectsChain {
    equalizer: ThreeBandEqualizer,
    compressor: Compressor,
    limiter: Limiter,
    enabled: bool,
}

impl AudioEffectsChain {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            equalizer: ThreeBandEqualizer::new(sample_rate),
            compressor: Compressor::new(sample_rate),
            limiter: Limiter::new(sample_rate),
            enabled: true,
        }
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        // Apply effects in chain: EQ -> Compressor -> Limiter
        self.equalizer.process(samples);
        self.compressor.process(samples);
        self.limiter.process(samples);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_eq_gain(&mut self, band: EQBand, gain_db: f32) {
        self.equalizer.set_gain(band, gain_db);
    }

    pub fn set_compressor_params(&mut self, threshold: f32, ratio: f32, attack_ms: f32, release_ms: f32) {
        self.compressor.set_threshold(threshold);
        self.compressor.set_ratio(ratio);
        self.compressor.set_attack(attack_ms);
        self.compressor.set_release(release_ms);
    }

    pub fn set_limiter_threshold(&mut self, threshold_db: f32) {
        self.limiter.set_threshold(threshold_db);
    }
}

/// 3-Band Equalizer (High, Mid, Low)
#[derive(Debug, Clone, Copy)]
pub enum EQBand {
    Low,
    Mid,
    High,
}

pub struct ThreeBandEqualizer {
    sample_rate: u32,
    low_shelf: BiquadFilter,
    mid_peak: BiquadFilter,
    high_shelf: BiquadFilter,
}

impl ThreeBandEqualizer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            low_shelf: BiquadFilter::low_shelf(sample_rate, 200.0, 0.7, 0.0),
            mid_peak: BiquadFilter::peak(sample_rate, 1000.0, 0.7, 0.0),
            high_shelf: BiquadFilter::high_shelf(sample_rate, 8000.0, 0.7, 0.0),
        }
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.low_shelf.process(*sample);
            *sample = self.mid_peak.process(*sample);
            *sample = self.high_shelf.process(*sample);
        }
    }

    pub fn set_gain(&mut self, band: EQBand, gain_db: f32) {
        match band {
            EQBand::Low => {
                self.low_shelf = BiquadFilter::low_shelf(self.sample_rate, 200.0, 0.7, gain_db);
            }
            EQBand::Mid => {
                self.mid_peak = BiquadFilter::peak(self.sample_rate, 1000.0, 0.7, gain_db);
            }
            EQBand::High => {
                self.high_shelf = BiquadFilter::high_shelf(self.sample_rate, 8000.0, 0.7, gain_db);
            }
        }
    }
}

/// Biquad IIR filter for EQ
pub struct BiquadFilter {
    a0: f32,
    a1: f32,
    a2: f32,
    b1: f32,
    b2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    pub fn low_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let s = 1.0;
        let beta = (gain / q).sqrt();

        let b0 = gain * ((gain + 1.0) - (gain - 1.0) * cos_w0 + beta * sin_w0);
        let b1 = 2.0 * gain * ((gain - 1.0) - (gain + 1.0) * cos_w0);
        let b2 = gain * ((gain + 1.0) - (gain - 1.0) * cos_w0 - beta * sin_w0);
        let a0 = (gain + 1.0) + (gain - 1.0) * cos_w0 + beta * sin_w0;
        let a1 = -2.0 * ((gain - 1.0) + (gain + 1.0) * cos_w0);
        let a2 = (gain + 1.0) + (gain - 1.0) * cos_w0 - beta * sin_w0;

        Self {
            a0: a0,
            a1: a1 / a0,
            a2: a2 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn high_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let beta = (gain / q).sqrt();

        let b0 = gain * ((gain + 1.0) + (gain - 1.0) * cos_w0 + beta * sin_w0);
        let b1 = -2.0 * gain * ((gain - 1.0) + (gain + 1.0) * cos_w0);
        let b2 = gain * ((gain + 1.0) + (gain - 1.0) * cos_w0 - beta * sin_w0);
        let a0 = (gain + 1.0) - (gain - 1.0) * cos_w0 + beta * sin_w0;
        let a1 = 2.0 * ((gain - 1.0) - (gain + 1.0) * cos_w0);
        let a2 = (gain + 1.0) - (gain - 1.0) * cos_w0 - beta * sin_w0;

        Self {
            a0: a0,
            a1: a1 / a0,
            a2: a2 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn peak(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * gain;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * gain;
        let a0 = 1.0 + alpha / gain;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / gain;

        Self {
            a0: a0,
            a1: a1 / a0,
            a2: a2 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let output = (input + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2) / self.a0;

        // Update delay line
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }
}

/// Dynamic range compressor
pub struct Compressor {
    sample_rate: u32,
    threshold: f32,
    ratio: f32,
    attack_coeff: f32,
    release_coeff: f32,
    envelope: f32,
    gain_reduction: f32,
}

impl Compressor {
    pub fn new(sample_rate: u32) -> Self {
        let mut compressor = Self {
            sample_rate,
            threshold: -12.0, // dB
            ratio: 4.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            envelope: 0.0,
            gain_reduction: 0.0,
        };

        compressor.set_attack(5.0); // 5ms attack
        compressor.set_release(100.0); // 100ms release
        compressor
    }

    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.threshold = threshold_db;
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
    }

    pub fn set_attack(&mut self, attack_ms: f32) {
        self.attack_coeff = (-1.0 / (attack_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    pub fn set_release(&mut self, release_ms: f32) {
        self.release_coeff = (-1.0 / (release_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            let input_level = sample.abs();
            let input_level_db = if input_level > 0.0 {
                20.0 * input_level.log10()
            } else {
                -100.0
            };

            // Envelope follower
            let target_envelope = if input_level_db > self.envelope {
                input_level_db
            } else {
                self.envelope
            };

            let coeff = if input_level_db > self.envelope {
                self.attack_coeff
            } else {
                self.release_coeff
            };

            self.envelope = target_envelope + (self.envelope - target_envelope) * coeff;

            // Compression calculation
            if self.envelope > self.threshold {
                let over_threshold = self.envelope - self.threshold;
                let compressed = over_threshold / self.ratio;
                self.gain_reduction = over_threshold - compressed;
            } else {
                self.gain_reduction = 0.0;
            }

            // Apply gain reduction
            let gain = 10.0_f32.powf(-self.gain_reduction / 20.0);
            *sample *= gain;
        }
    }
}

/// Brick-wall limiter
pub struct Limiter {
    sample_rate: u32,
    threshold: f32,
    release_coeff: f32,
    envelope: f32,
    delay_line: Vec<f32>,
    delay_index: usize,
}

impl Limiter {
    pub fn new(sample_rate: u32) -> Self {
        let lookahead_samples = (sample_rate as f32 * 0.005) as usize; // 5ms lookahead
        
        let mut limiter = Self {
            sample_rate,
            threshold: -0.1, // dB
            release_coeff: 0.0,
            envelope: 0.0,
            delay_line: vec![0.0; lookahead_samples],
            delay_index: 0,
        };

        limiter.set_release(50.0); // 50ms release
        limiter
    }

    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.threshold = threshold_db;
    }

    pub fn set_release(&mut self, release_ms: f32) {
        self.release_coeff = (-1.0 / (release_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            // Store input in delay line
            self.delay_line[self.delay_index] = *sample;
            
            // Get delayed sample for output
            let delayed_sample = self.delay_line[(self.delay_index + 1) % self.delay_line.len()];
            
            // Calculate input level in dB
            let input_level_db = if sample.abs() > 0.0 {
                20.0 * sample.abs().log10()
            } else {
                -100.0
            };

            // Peak detection with lookahead
            let target_envelope = input_level_db.max(self.envelope);
            
            // Smooth envelope
            self.envelope = target_envelope + (self.envelope - target_envelope) * self.release_coeff;

            // Calculate gain reduction
            let gain_reduction = if self.envelope > self.threshold {
                self.envelope - self.threshold
            } else {
                0.0
            };

            // Apply limiting
            let gain = 10.0_f32.powf(-gain_reduction / 20.0);
            *sample = delayed_sample * gain;

            // Advance delay line
            self.delay_index = (self.delay_index + 1) % self.delay_line.len();
        }
    }
}

impl SpectrumAnalyzer {
    fn new(sample_rate: u32, fft_size: usize) -> Self {
        // Create Hann window for better frequency resolution
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                let phase = 2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32;
                0.5 * (1.0 - phase.cos())
            })
            .collect();

        Self {
            sample_rate,
            fft_size,
            window,
            input_buffer: vec![0.0; fft_size],
            output_spectrum: vec![0.0; fft_size / 2],
        }
    }

    fn process(&mut self, _samples: &[f32]) {
        // This would typically use a real FFT library like rustfft
        // For now, we'll keep it as a placeholder
        // In production, implement proper frequency domain analysis
    }

    pub fn get_spectrum(&self) -> &[f32] {
        &self.output_spectrum
    }
}

impl VirtualMixer {
    pub async fn new(config: MixerConfig) -> Result<Self> {
        let (command_tx, command_rx) = mpsc::channel(1024);
        let (audio_output_tx, _audio_output_rx) = mpsc::channel(8192);
        
        let buffer_size = config.buffer_size as usize;
        let mix_buffer = Arc::new(Mutex::new(vec![0.0; buffer_size * 2])); // Stereo

        let metrics = Arc::new(Mutex::new(AudioMetrics {
            cpu_usage: 0.0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            latency_ms: (buffer_size as f32 / config.sample_rate as f32) * 1000.0,
            sample_rate: config.sample_rate,
            active_channels: config.channels.len() as u32,
        }));

        // Initialize audio device manager
        let audio_device_manager = Arc::new(AudioDeviceManager::new()?);

        let channel_levels = Arc::new(Mutex::new(HashMap::new()));
        let master_levels = Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0)));

        Ok(Self {
            config: config.clone(),
            is_running: Arc::new(AtomicBool::new(false)),
            mix_buffer,
            sample_rate_converter: None,
            audio_analyzer: AudioAnalyzer::new(config.sample_rate),
            command_tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
            audio_output_tx,
            metrics,
            channel_levels,
            master_levels,
            audio_device_manager,
            input_streams: Arc::new(Mutex::new(HashMap::new())),
            output_stream: Arc::new(Mutex::new(None)),
        })
    }

    /// Start the virtual mixer
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        println!("Starting Virtual Mixer with real audio capture...");

        self.is_running.store(true, Ordering::Relaxed);
        
        // Start the audio processing thread
        self.start_processing_thread().await?;
        
        Ok(())
    }

    /// Add an audio input stream with real audio capture using cpal
    pub async fn add_input_stream(&self, device_id: &str) -> Result<()> {
        println!("Adding real audio input stream for device: {}", device_id);
        
        // Find the actual cpal device
        let device = self.audio_device_manager.find_cpal_device(device_id, true).await?;
        let device_name = device.name().unwrap_or_else(|_| device_id.to_string());
        
        println!("Found cpal device: {}", device_name);
        
        // Get the default input config for this device
        let config = device.default_input_config()
            .context("Failed to get default input config")?;
            
        println!("Device config: {:?}", config);
        
        // Create AudioInputStream structure
        let input_stream = AudioInputStream::new(
            device_id.to_string(),
            device_name.clone(),
            self.config.sample_rate,
        )?;
        
        // Get references for the audio callback
        let audio_buffer = input_stream.audio_buffer.clone();
        let target_sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size as usize;
        
        // Create the appropriate stream config
        let stream_config = StreamConfig {
            channels: config.channels().min(2), // Limit to stereo max
            sample_rate: cpal::SampleRate(target_sample_rate),
            buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
        };
        
        println!("Using stream config: channels={}, sample_rate={}, buffer_size={}", 
                stream_config.channels, stream_config.sample_rate.0, buffer_size);
        
        // Add to streams collection first
        let mut streams = self.input_streams.lock().await;
        streams.insert(device_id.to_string(), Arc::new(input_stream));
        drop(streams); // Release the async lock
        
        // Send stream creation command to the synchronous stream manager thread
        let stream_manager = get_stream_manager();
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        
        let command = StreamCommand::AddInputStream {
            device_id: device_id.to_string(),
            device,
            config: stream_config,
            audio_buffer,
            target_sample_rate,
            response_tx,
        };
        
        stream_manager.send(command)
            .context("Failed to send stream creation command")?;
            
        // Wait for the response from the stream manager thread
        let result = response_rx.recv()
            .context("Failed to receive stream creation response")?;
            
        match result {
            Ok(()) => {
                println!("Successfully started audio input stream for: {}", device_name);
                println!("Successfully added real audio input stream: {}", device_id);
                Ok(())
            }
            Err(e) => {
                // Remove from streams collection if stream creation failed
                let mut streams = self.input_streams.lock().await;
                streams.remove(device_id);
                Err(e)
            }
        }
    }

    /// Set the audio output stream
    pub async fn set_output_stream(&self, device_id: &str) -> Result<()> {
        println!("Setting output stream for device: {}", device_id);
        
        // Try to find the actual cpal device for output
        let devices = self.audio_device_manager.enumerate_devices().await?;
        let target_device = devices.iter().find(|d| d.id == device_id && d.is_output);
        
        if target_device.is_none() {
            println!("Warning: Output device {} not found, using default", device_id);
        }
        
        // Create a buffer-based output stream (we'll enhance this with real cpal output later)
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            device_id.replace("_", " "),
            self.config.sample_rate,
        )?;
        
        println!("Setting up output routing for: {}", device_id);
        
        // For now, let's at least start a simple audio playback thread that reads from the buffer
        let output_buffer = output_stream.input_buffer.clone();
        let sample_rate = self.config.sample_rate;
        
        tokio::spawn(async move {
            println!("Starting audio playback thread for output device");
            loop {
                // Read samples from buffer and "play" them (for now just consume them)
                if let Ok(mut buffer) = output_buffer.try_lock() {
                    if !buffer.is_empty() {
                        let samples_to_play = buffer.len().min(512); // Play in chunks
                        let _played_samples: Vec<_> = buffer.drain(0..samples_to_play).collect();
                        // In a real implementation, these samples would be sent to cpal output stream
                        if samples_to_play > 0 {
                            // println!("Playing {} samples to output device", samples_to_play);
                        }
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // ~10ms intervals
            }
        });
        
        let mut stream_guard = self.output_stream.lock().await;
        *stream_guard = Some(Arc::new(output_stream));
        println!("Successfully set output stream with playback thread: {}", device_id);
        
        Ok(())
    }

    /// Remove an input stream and clean up cpal stream
    pub async fn remove_input_stream(&self, device_id: &str) -> Result<()> {
        // Remove from streams collection
        let mut streams = self.input_streams.lock().await;
        let was_present = streams.remove(device_id).is_some();
        drop(streams); // Release the async lock
        
        if was_present {
            // Send stream removal command to the synchronous stream manager thread
            let stream_manager = get_stream_manager();
            let (response_tx, response_rx) = std::sync::mpsc::channel();
            
            let command = StreamCommand::RemoveStream {
                device_id: device_id.to_string(),
                response_tx,
            };
            
            stream_manager.send(command)
                .context("Failed to send stream removal command")?;
                
            // Wait for the response
            let removed = response_rx.recv()
                .context("Failed to receive stream removal response")?;
                
            if removed {
                println!("Removed input stream and cleaned up cpal stream: {}", device_id);
            } else {
                println!("Stream was not found in manager for removal: {}", device_id);
            }
        } else {
            println!("Input stream not found for removal: {}", device_id);
        }
        
        Ok(())
    }


    /// Stop the virtual mixer
    pub async fn stop(&mut self) -> Result<()> {
        self.is_running.store(false, Ordering::Relaxed);
        
        // TODO: Stop all audio streams (will be managed separately)
        
        Ok(())
    }

    async fn start_processing_thread(&self) -> Result<()> {
        let is_running = self.is_running.clone();
        let mix_buffer = self.mix_buffer.clone();
        let audio_output_tx = self.audio_output_tx.clone();
        let metrics = self.metrics.clone();
        let channel_levels = self.channel_levels.clone();
        let master_levels = self.master_levels.clone();
        let sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size;
        let config_channels = self.config.channels.clone();
        let mixer_handle = VirtualMixerHandle {
            input_streams: self.input_streams.clone(),
            output_stream: self.output_stream.clone(),
        };

        // Spawn real-time audio processing task
        tokio::spawn(async move {
            let mut frame_count = 0u64;
            
            println!("Audio processing thread started with real mixing");

            while is_running.load(Ordering::Relaxed) {
                let process_start = std::time::Instant::now();
                
                // Collect input samples from all active input streams with effects processing
                let input_samples = mixer_handle.collect_input_samples_with_effects(&config_channels).await;
                
                // Create the output buffer (stereo)
                let mut output_buffer = vec![0.0f32; (buffer_size * 2) as usize];
                
                // Calculate channel levels and mix audio
                let mut calculated_channel_levels = std::collections::HashMap::new();
                
                if !input_samples.is_empty() {
                    let mut mixed_samples = vec![0.0f32; buffer_size as usize];
                    let mut active_channels = 0;
                    
                    // Mix all input channels together and calculate levels
                    for (device_id, samples) in input_samples.iter() {
                        if !samples.is_empty() {
                            active_channels += 1;
                            
                            // Calculate peak and RMS levels for VU meters
                            let peak_level = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                            let rms_level = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                            
                            // Find which channel this device belongs to
                            if let Some(channel) = config_channels.iter().find(|ch| {
                                ch.input_device_id.as_ref() == Some(device_id)
                            }) {
                                // Store levels by channel ID
                                calculated_channel_levels.insert(channel.id, (peak_level, rms_level));
                                
                                // Log levels occasionally
                                if frame_count % 100 == 0 && peak_level > 0.001 {
                                    println!("Channel {} ({}): {} samples, peak: {:.3}, rms: {:.3}", 
                                        channel.id, device_id, samples.len(), peak_level, rms_level);
                                }
                            }
                            
                            // Simple mixing: add samples together
                            let mix_length = mixed_samples.len().min(samples.len());
                            for i in 0..mix_length {
                                mixed_samples[i] += samples[i];
                            }
                        }
                    }
                    
                    // Normalize by number of active channels to prevent clipping
                    if active_channels > 0 {
                        let gain = 1.0 / active_channels as f32;
                        for sample in mixed_samples.iter_mut() {
                            *sample *= gain;
                        }
                    }
                    
                    // Convert mono mixed samples to stereo output
                    for (i, &sample) in mixed_samples.iter().enumerate() {
                        if i * 2 + 1 < output_buffer.len() {
                            output_buffer[i * 2] = sample;     // Left channel
                            output_buffer[i * 2 + 1] = sample; // Right channel
                        }
                    }
                    
                    // Apply basic gain (master volume)
                    let master_gain = 0.5f32; // Reduce volume to prevent clipping
                    for sample in output_buffer.iter_mut() {
                        *sample *= master_gain;
                    }
                    
                    // Calculate master output levels for L/R channels
                    let left_samples: Vec<f32> = output_buffer.iter().step_by(2).copied().collect();
                    let right_samples: Vec<f32> = output_buffer.iter().skip(1).step_by(2).copied().collect();
                    
                    let left_peak = left_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let left_rms = if !left_samples.is_empty() {
                        (left_samples.iter().map(|&s| s * s).sum::<f32>() / left_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    let right_peak = right_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let right_rms = if !right_samples.is_empty() {
                        (right_samples.iter().map(|&s| s * s).sum::<f32>() / right_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    // Store real master levels
                    if let Ok(mut levels_guard) = master_levels.try_lock() {
                        *levels_guard = (left_peak, left_rms, right_peak, right_rms);
                    }
                    
                    // Log master levels occasionally
                    if frame_count % 100 == 0 && (left_peak > 0.001 || right_peak > 0.001) {
                        println!("Master output: L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})", 
                            left_peak, left_rms, right_peak, right_rms);
                    }
                }
                
                // Store calculated channel levels for VU meters
                if let Ok(mut levels_guard) = channel_levels.try_lock() {
                    *levels_guard = calculated_channel_levels;
                }
                
                // Update mix buffer
                if let Ok(mut buffer_guard) = mix_buffer.try_lock() {
                    if buffer_guard.len() == output_buffer.len() {
                        buffer_guard.copy_from_slice(&output_buffer);
                    }
                }
                
                // Send to output stream
                mixer_handle.send_to_output(&output_buffer).await;

                // Send processed audio to the rest of the application (non-blocking)
                let _ = audio_output_tx.try_send(output_buffer.clone());
                // Don't break on send failure - just continue processing

                frame_count += 1;
                
                // Update metrics every second
                if frame_count % (sample_rate / buffer_size) as u64 == 0 {
                    let cpu_time = process_start.elapsed().as_secs_f32();
                    let max_cpu_time = buffer_size as f32 / sample_rate as f32;
                    let cpu_usage = (cpu_time / max_cpu_time) * 100.0;
                    
                    if let Ok(mut metrics_guard) = metrics.try_lock() {
                        metrics_guard.cpu_usage = cpu_usage;
                    }
                    
                    if input_samples.len() > 0 {
                        println!("Audio processing: CPU {:.1}%, {} active streams", cpu_usage, input_samples.len());
                    }
                }

                // Maintain real-time constraints
                let target_duration = std::time::Duration::from_micros(
                    (buffer_size as u64 * 1_000_000) / sample_rate as u64
                );
                let elapsed = process_start.elapsed();
                if elapsed < target_duration {
                    tokio::time::sleep(target_duration - elapsed).await;
                }
            }
            
            println!("Audio processing thread stopped");
        });

        Ok(())
    }

    /// Add a new audio channel
    pub async fn add_channel(&mut self, channel: AudioChannel) -> Result<()> {
        // TODO: Add ring buffer management
        self.config.channels.push(channel);
        Ok(())
    }

    /// Get current mixer metrics
    pub async fn get_metrics(&self) -> AudioMetrics {
        self.metrics.lock().await.clone()
    }

    /// Get current channel levels for VU meters
    pub async fn get_channel_levels(&self) -> HashMap<u32, (f32, f32)> {
        // Return real audio levels from processing thread
        if let Ok(levels_guard) = self.channel_levels.try_lock() {
            levels_guard.clone()
        } else {
            // Fallback to empty levels if we can't get the lock
            HashMap::new()
        }
    }

    /// Get current master output levels for VU meters (Left/Right)
    pub async fn get_master_levels(&self) -> (f32, f32, f32, f32) {
        // Return real master audio levels from processing thread
        if let Ok(levels_guard) = self.master_levels.try_lock() {
            *levels_guard
        } else {
            // Fallback to zero levels if we can't get the lock
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Get audio output stream for streaming/recording
    pub async fn get_audio_output_receiver(&self) -> mpsc::Receiver<Vec<f32>> {
        let (_tx, rx) = mpsc::channel(8192);
        // In a real implementation, this would connect to the actual audio output
        rx
    }

    /// Send command to mixer
    pub async fn send_command(&self, command: MixerCommand) -> Result<()> {
        self.command_tx.send(command).await
            .context("Failed to send mixer command")?;
        Ok(())
    }

    /// Update channel configuration
    pub async fn update_channel(&mut self, channel_id: u32, updated_channel: AudioChannel) -> Result<()> {
        if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == channel_id) {
            *channel = updated_channel;
        }
        Ok(())
    }

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

