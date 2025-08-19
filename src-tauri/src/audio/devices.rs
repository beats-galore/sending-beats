use anyhow::{Context, Result};
use cpal::{Device, Host};
use cpal::traits::{DeviceTrait, HostTrait};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(target_os = "macos")]
use coreaudio_sys::{AudioDeviceID, kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMaster, kAudioDevicePropertyDeviceNameCFString, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeOutput, kAudioObjectPropertyScopeInput, kAudioObjectSystemObject, AudioObjectPropertyAddress};
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;

use super::types::{AudioDeviceInfo, AudioDeviceHandle};

#[cfg(target_os = "macos")]
use super::types::CoreAudioDevice;

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

    /// Get ALL audio devices including hardware bypassed by system routing
    pub async fn enumerate_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        println!("Starting comprehensive audio device enumeration...");
        
        let mut all_devices = Vec::new();
        
        // First, get devices through CoreAudio directly (macOS only)
        #[cfg(target_os = "macos")]
        {
            println!("=== DIRECT COREAUDIO ENUMERATION ===");
            match self.enumerate_coreaudio_devices().await {
                Ok(coreaudio_devices) => {
                    println!("Found {} devices via direct CoreAudio access", coreaudio_devices.len());
                    all_devices.extend(coreaudio_devices);
                }
                Err(e) => {
                    println!("CoreAudio direct access failed: {}, falling back to cpal", e);
                }
            }
        }
        
        // Then supplement with cpal devices
        println!("\n=== CPAL ENUMERATION (SUPPLEMENTAL) ===");
        match self.enumerate_cpal_devices().await {
            Ok(cpal_devices) => {
                println!("Found {} devices via cpal", cpal_devices.len());
                
                // Add cpal devices that aren't already in our list
                for cpal_device in cpal_devices {
                    if !all_devices.iter().any(|existing| existing.name == cpal_device.name) {
                        all_devices.push(cpal_device);
                    }
                }
            }
            Err(e) => {
                println!("cpal enumeration failed: {}", e);
            }
        }
        
        println!("\n=== FINAL DEVICE LIST ===");
        for (i, device) in all_devices.iter().enumerate() {
            println!("  {}: {} ({})", i, device.name, device.id);
            println!("     Input: {}, Output: {}, Default: {}", 
                device.is_input, device.is_output, device.is_default);
        }
        
        // Update cache with all devices (CoreAudio + cpal)
        {
            let mut cache_guard = self.devices_cache.lock().await;
            for device in &all_devices {
                cache_guard.insert(device.id.clone(), device.clone());
            }
        }
        
        Ok(all_devices)
    }

    /// Enumerate devices using direct CoreAudio API access
    #[cfg(target_os = "macos")]
    async fn enumerate_coreaudio_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        use std::ptr;
        use std::mem;
        
        let mut devices = Vec::new();
        
        // Get all audio devices from CoreAudio
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };
        
        // Get the number of devices
        let mut data_size: u32 = 0;
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyDataSize(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut data_size as *mut _
            )
        };
        
        if status != 0 {
            return Err(anyhow::anyhow!("Failed to get CoreAudio device count: {}", status));
        }
        
        let device_count = data_size / mem::size_of::<AudioDeviceID>() as u32;
        println!("CoreAudio reports {} total audio devices", device_count);
        
        if device_count == 0 {
            return Ok(devices);
        }
        
        // Get the device IDs
        let mut device_ids: Vec<AudioDeviceID> = vec![0; device_count as usize];
        let mut actual_size = data_size;
        
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut actual_size as *mut _,
                device_ids.as_mut_ptr() as *mut _
            )
        };
        
        if status != 0 {
            return Err(anyhow::anyhow!("Failed to get CoreAudio device IDs: {}", status));
        }
        
        // Process each device
        for device_id in device_ids {
            match self.get_coreaudio_device_info(device_id).await {
                Ok(Some(device_info)) => {
                    println!("  Found CoreAudio device: {} ({})", device_info.name, device_info.id);
                    devices.push(device_info);
                }
                Ok(None) => {
                    // Device filtered out (e.g., no streams)
                }
                Err(e) => {
                    println!("  Failed to get info for device {}: {}", device_id, e);
                }
            }
        }
        
        Ok(devices)
    }
    
    /// Get device info from CoreAudio device ID
    #[cfg(target_os = "macos")]
    async fn get_coreaudio_device_info(&self, device_id: AudioDeviceID) -> Result<Option<AudioDeviceInfo>> {
        use std::ptr;
        use std::mem;
        
        // Get device name and convert to String immediately to avoid Send issues
        let device_name = {
            let name_property = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyDeviceNameCFString,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMaster,
            };
            
            let mut name_size = mem::size_of::<CFStringRef>() as u32;
            let mut cf_string_ref: CFStringRef = ptr::null();
            
            let status = unsafe {
                coreaudio_sys::AudioObjectGetPropertyData(
                    device_id,
                    &name_property as *const _,
                    0,
                    ptr::null(),
                    &mut name_size as *mut _,
                    &mut cf_string_ref as *mut _ as *mut _
                )
            };
            
            if status != 0 {
                return Err(anyhow::anyhow!("Failed to get device name for device {}: {}", device_id, status));
            }
            
            let cf_string = unsafe { CFString::wrap_under_get_rule(cf_string_ref) };
            cf_string.to_string()
        };
        
        // Check if device has output streams
        let output_streams_property = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyStreams,
            mScope: kAudioObjectPropertyScopeOutput,
            mElement: kAudioObjectPropertyElementMaster,
        };
        
        let mut output_streams_size: u32 = 0;
        let output_status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyDataSize(
                device_id,
                &output_streams_property as *const _,
                0,
                ptr::null(),
                &mut output_streams_size as *mut _
            )
        };
        
        let has_output = output_status == 0 && output_streams_size > 0;
        
        // Check if device has input streams
        let input_streams_property = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyStreams,
            mScope: kAudioObjectPropertyScopeInput,
            mElement: kAudioObjectPropertyElementMaster,
        };
        
        let mut input_streams_size: u32 = 0;
        let input_status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyDataSize(
                device_id,
                &input_streams_property as *const _,
                0,
                ptr::null(),
                &mut input_streams_size as *mut _
            )
        };
        
        let has_input = input_status == 0 && input_streams_size > 0;
        
        // Skip devices that have neither input nor output
        if !has_input && !has_output {
            return Ok(None);
        }
        
        // Generate device ID
        let device_type = if has_output { "output" } else { "input" };
        let clean_name = device_name.replace(" ", "_").replace("(", "").replace(")", "").to_lowercase();
        let device_id_string = format!("{}_{}", device_type, clean_name);
        
        // Check if this is a default device
        let is_default = self.is_coreaudio_default_device(device_id, has_output).await.unwrap_or(false);
        
        let device_info = AudioDeviceInfo {
            id: device_id_string,
            name: device_name,
            is_input: has_input,
            is_output: has_output,
            is_default,
            supported_sample_rates: vec![44100, 48000], // Default rates
            supported_channels: vec![2], // Assume stereo
            host_api: "CoreAudio (Direct)".to_string(),
        };
        
        Ok(Some(device_info))
    }
    
    /// Check if a CoreAudio device is the system default
    #[cfg(target_os = "macos")]
    async fn is_coreaudio_default_device(&self, device_id: AudioDeviceID, is_output: bool) -> Result<bool> {
        use coreaudio_sys::{kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDefaultInputDevice};
        use std::mem;
        
        let property_selector = if is_output {
            kAudioHardwarePropertyDefaultOutputDevice
        } else {
            kAudioHardwarePropertyDefaultInputDevice
        };
        
        let property = AudioObjectPropertyAddress {
            mSelector: property_selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };
        
        let mut default_device_id: AudioDeviceID = 0;
        let mut size = mem::size_of::<AudioDeviceID>() as u32;
        
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &property as *const _,
                0,
                std::ptr::null(),
                &mut size as *mut _,
                &mut default_device_id as *mut _ as *mut _
            )
        };
        
        if status == 0 {
            Ok(default_device_id == device_id)
        } else {
            Ok(false)
        }
    }

    /// Fallback enumeration using cpal (existing method, renamed)
    async fn enumerate_cpal_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        println!("Starting cpal device enumeration...");
        
        // Debug: Check all available hosts and their devices
        println!("Available cpal hosts:");
        for host_id in cpal::ALL_HOSTS {
            println!("  - {:?}", host_id);
        }
        
        println!("Current host: {:?}", self.host.id());
        
        // Check what cpal can see vs what's actually available in the system
        #[cfg(target_os = "macos")]
        {
            println!("=== CPAL Device Detection Debug ===");
            
            // Check all hosts to see if any see more devices
            for host_id in cpal::ALL_HOSTS {
                match cpal::host_from_id(*host_id) {
                    Ok(host) => {
                        println!("Host {:?}:", host_id);
                        
                        match host.output_devices() {
                            Ok(devices) => {
                                let devices_vec: Vec<_> = devices.collect();
                                println!("  CPAL Output devices: {}", devices_vec.len());
                                for (i, device) in devices_vec.iter().enumerate() {
                                    let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                                    println!("    {}: {} {:?}", i, name, device.name());
                                    
                                    // Try to get device configs to see why some might be filtered
                                    match device.supported_output_configs() {
                                        Ok(configs) => {
                                            let config_count = configs.count();
                                            println!("      -> {} supported configs", config_count);
                                        }
                                        Err(e) => {
                                            println!("      -> Config error: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                println!("  CPAL output devices error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  Host {:?} init failed: {}", host_id, e);
                    }
                }
            }
            
            // Let's try to access CoreAudio directly to see what's really available
            println!("\n=== System Profiler Shows These Devices ===");
            println!("- BenQ EW3270U (DisplayPort)");
            println!("- External Headphones (Built-in)"); 
            println!("- MacBook Pro Speakers (Built-in)");
            println!("- BlackHole 2ch (Virtual) - DEFAULT");
            println!("- Serato Virtual Audio (Virtual)");
            
            println!("\n=== Possible Issues ===");
            println!("1. cpal only shows 'active' devices");
            println!("2. Physical devices hidden when virtual device is default");
            println!("3. App permissions limiting device visibility");
            println!("4. CoreAudio routing configuration");
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

        // Add system default devices if they exist and weren't already detected
        #[cfg(target_os = "macos")]
        {
            // Only add actual system defaults if they exist
            if let Some(default_out) = default_output {
                let default_name = default_out.name().unwrap_or_else(|_| "System Default Output".to_string());
                let default_id = "output_system_default";
                
                if !devices_map.contains_key(default_id) {
                    println!("Adding system default output device: {}", default_name);
                    if let Ok(default_device_info) = self.get_device_info(&default_out, false, true, true) {
                        let mut system_default = default_device_info;
                        system_default.id = default_id.to_string();
                        system_default.name = format!("{} (Default)", system_default.name);
                        devices.push(system_default.clone());
                        devices_map.insert(default_id.to_string(), system_default);
                    }
                }
            }
            
            if let Some(default_in) = default_input {
                let default_name = default_in.name().unwrap_or_else(|_| "System Default Input".to_string());
                let default_id = "input_system_default";
                
                if !devices_map.contains_key(default_id) {
                    println!("Adding system default input device: {}", default_name);
                    if let Ok(default_device_info) = self.get_device_info(&default_in, true, false, true) {
                        let mut system_default = default_device_info;
                        system_default.id = default_id.to_string();
                        system_default.name = format!("{} (Default)", system_default.name);
                        devices.push(system_default.clone());
                        devices_map.insert(default_id.to_string(), system_default);
                    }
                }
            }
        }

        // Don't replace the entire cache - this would wipe out CoreAudio devices!
        // Instead, update the cache by merging cpal devices
        {
            let mut cache_guard = self.devices_cache.lock().await;
            for (device_id, device_info) in devices_map {
                cache_guard.insert(device_id, device_info);
            }
        }

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
    
    /// Find audio device for streaming - tries CoreAudio first, then cpal fallback
    pub async fn find_audio_device(&self, device_id: &str, is_input: bool) -> Result<AudioDeviceHandle> {
        println!("Searching for audio device: {} (input: {})", device_id, is_input);
        
        // First try to find the device in our cache
        if let Some(device_info) = self.get_device(device_id).await {
            if device_info.host_api == "CoreAudio (Direct)" {
                println!("Found CoreAudio device: {}", device_info.name);
                return self.create_coreaudio_device_handle(&device_info, is_input).await;
            }
        }
        
        // If not found in cache, refresh the device list and try again
        println!("Device not found in cache, refreshing device list...");
        let _refreshed_devices = self.enumerate_devices().await?;
        
        if let Some(device_info) = self.get_device(device_id).await {
            if device_info.host_api == "CoreAudio (Direct)" {
                println!("Found CoreAudio device after refresh: {}", device_info.name);
                return self.create_coreaudio_device_handle(&device_info, is_input).await;
            }
        }
        
        // Fallback to cpal device search
        match self.find_cpal_device(device_id, is_input).await {
            Ok(cpal_device) => {
                println!("Found cpal device: {}", device_id);
                Ok(AudioDeviceHandle::Cpal(cpal_device))
            }
            Err(e) => {
                println!("No device found for '{}': {}", device_id, e);
                Err(e)
            }
        }
    }

    /// Create a CoreAudio device handle for direct audio streaming
    #[cfg(target_os = "macos")]
    async fn create_coreaudio_device_handle(&self, device_info: &AudioDeviceInfo, _is_input: bool) -> Result<AudioDeviceHandle> {
        // Extract the actual CoreAudio device ID from our device info
        // We need to re-enumerate to get the raw device ID
        match self.find_coreaudio_device_id(&device_info.name).await {
            Ok(device_id) => {
                println!("Creating CoreAudio handle for device {} (ID: {})", device_info.name, device_id);
                Ok(AudioDeviceHandle::CoreAudio(CoreAudioDevice {
                    device_id,
                    name: device_info.name.clone(),
                    sample_rate: 44100, // Default
                    channels: 2,        // Default stereo
                    stream: None,       // Stream will be created when needed
                }))
            }
            Err(e) => {
                println!("Failed to find CoreAudio device ID for {}: {}", device_info.name, e);
                Err(e)
            }
        }
    }

    /// Find the CoreAudio device ID by name
    #[cfg(target_os = "macos")]
    async fn find_coreaudio_device_id(&self, device_name: &str) -> Result<AudioDeviceID> {
        use std::ptr;
        use std::mem;
        
        // Get all audio devices from CoreAudio
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };
        
        // Get device count
        let mut data_size: u32 = 0;
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyDataSize(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut data_size as *mut _
            )
        };
        
        if status != 0 {
            return Err(anyhow::anyhow!("Failed to get CoreAudio device count: {}", status));
        }
        
        let device_count = data_size / mem::size_of::<AudioDeviceID>() as u32;
        let mut device_ids: Vec<AudioDeviceID> = vec![0; device_count as usize];
        let mut actual_size = data_size;
        
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut actual_size as *mut _,
                device_ids.as_mut_ptr() as *mut _
            )
        };
        
        if status != 0 {
            return Err(anyhow::anyhow!("Failed to get CoreAudio device IDs: {}", status));
        }
        
        // Find device by name
        for device_id in device_ids {
            if let Ok(name) = self.get_coreaudio_device_name(device_id).await {
                if name == device_name {
                    return Ok(device_id);
                }
            }
        }
        
        Err(anyhow::anyhow!("CoreAudio device not found: {}", device_name))
    }

    /// Get CoreAudio device name by ID
    #[cfg(target_os = "macos")]
    async fn get_coreaudio_device_name(&self, device_id: AudioDeviceID) -> Result<String> {
        use std::ptr;
        use std::mem;
        
        let name_property = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyDeviceNameCFString,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };
        
        let mut name_size = mem::size_of::<CFStringRef>() as u32;
        let mut cf_string_ref: CFStringRef = ptr::null();
        
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                device_id,
                &name_property as *const _,
                0,
                ptr::null(),
                &mut name_size as *mut _,
                &mut cf_string_ref as *mut _ as *mut _
            )
        };
        
        if status != 0 {
            return Err(anyhow::anyhow!("Failed to get device name for device {}: {}", device_id, status));
        }
        
        let cf_string = unsafe { CFString::wrap_under_get_rule(cf_string_ref) };
        Ok(cf_string.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    async fn create_coreaudio_device_handle(&self, _device_info: &AudioDeviceInfo, _is_input: bool) -> Result<AudioDeviceHandle> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
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
        
        // As a last resort, check if this is a request for system default
        if device_id.contains("system_default") || device_id.contains("default") {
            println!("Attempting to use system default device for: {}", device_id);
            if is_input {
                if let Some(default_device) = self.host.default_input_device() {
                    println!("Using system default input device");
                    return Ok(default_device);
                }
            } else {
                if let Some(default_device) = self.host.default_output_device() {
                    println!("Using system default output device");
                    return Ok(default_device);
                }
            }
        }
        
        // Device not found
        println!("No device found for '{}' after all matching attempts", device_id);
        
        Err(anyhow::anyhow!("No suitable {} device found for ID: {}", 
            if is_input { "input" } else { "output" }, device_id))
    }
}