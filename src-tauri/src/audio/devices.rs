use anyhow::{Context, Result};
use cpal::{Device, Host};
use cpal::traits::{DeviceTrait, HostTrait};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::types::AudioDeviceInfo;

/// Cross-platform audio device manager
pub struct AudioDeviceManager {
    host: Host,
    devices_cache: Arc<Mutex<HashMap<String, AudioDeviceInfo>>>,
}

impl std::fmt::Debug for AudioDeviceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioDeviceManager")
            .field("host", &"Host(cpal)")
            .field("devices_cache", &self.devices_cache)
            .finish()
    }
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
        
        // Validate device name
        if name.len() > 512 {
            return Err(anyhow::anyhow!("Device name too long: {}", name.len()));
        }
        
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
                
                // Check if device name contains any of the target name parts (must be at least 3 characters)
                let matches = target_name_parts.iter().any(|&part| {
                    !part.is_empty() && part.len() >= 3 && name_lower.contains(part)
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
                
                // Check if device name contains any of the target name parts (must be at least 3 characters)
                let matches = target_name_parts.iter().any(|&part| {
                    !part.is_empty() && part.len() >= 3 && name_lower.contains(part)
                });
                
                if matches {
                    println!("Found partial match for output device: '{}' matches '{}'", device_name, device_id);
                    return Ok(device);
                }
            }
        }
        
        // Device not found - don't fall back to default devices automatically
        println!("No device found for '{}' after all matching attempts", device_id);
        
        Err(anyhow::anyhow!("No suitable {} device found for ID: {}", 
            if is_input { "input" } else { "output" }, device_id))
    }
}