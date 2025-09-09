// Device discovery and enumeration system
//
// This module handles the core device enumeration logic, including both
// CPAL-based enumeration and safe device discovery with crash protection.
// It provides device information extraction, name cleaning, and availability
// filtering.

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use crate::types::SUPPORTED_INPUT_SAMPLE_RATES_HZ;
use super::coreaudio_integration::CoreAudioIntegration;
use crate::audio::types::AudioDeviceInfo;

/// Device enumeration system with crash protection
pub struct DeviceEnumerator {
    host: Host,
    devices_cache: Arc<Mutex<HashMap<String, AudioDeviceInfo>>>,
    coreaudio: CoreAudioIntegration,
}

impl DeviceEnumerator {
    /// Create a new device enumerator
    pub fn new() -> Result<Self> {
        // Try to use CoreAudio host explicitly on macOS for better device detection
        #[cfg(target_os = "macos")]
        let host = {
            crate::device_debug!("Attempting to use CoreAudio host on macOS...");
            match cpal::host_from_id(cpal::HostId::CoreAudio) {
                Ok(host) => {
                    crate::device_debug!("Successfully initialized CoreAudio host");
                    host
                }
                Err(e) => {
                    warn!(
                        "Failed to initialize CoreAudio host: {}, falling back to default",
                        e
                    );
                    cpal::default_host()
                }
            }
        };

        #[cfg(not(target_os = "macos"))]
        let host = cpal::default_host();

        crate::device_debug!("Using audio host: {:?}", host.id());

        let devices_cache = Arc::new(Mutex::new(HashMap::new()));
        let coreaudio = CoreAudioIntegration::new(devices_cache.clone());

        Ok(Self {
            host,
            devices_cache,
            coreaudio,
        })
    }

    /// Get ALL audio devices including hardware bypassed by system routing
    pub async fn enumerate_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        crate::device_debug!("Starting comprehensive audio device enumeration...");

        let mut all_devices = Vec::new();

        // First, get devices through CoreAudio directly (macOS only)
        #[cfg(target_os = "macos")]
        {
            crate::device_debug!("=== DIRECT COREAUDIO ENUMERATION ===");
            match self.coreaudio.enumerate_coreaudio_devices().await {
                Ok(coreaudio_devices) => {
                    crate::device_debug!(
                        "Found {} devices via direct CoreAudio access",
                        coreaudio_devices.len()
                    );
                    all_devices.extend(coreaudio_devices);
                }
                Err(e) => {
                    warn!(
                        "CoreAudio direct access failed: {}, falling back to cpal",
                        e
                    );
                }
            }
        }

        // Then supplement with cpal devices
        crate::device_debug!("\n=== CPAL ENUMERATION (SUPPLEMENTAL) ===");
        match self.enumerate_cpal_devices().await {
            Ok(cpal_devices) => {
                crate::device_debug!("Found {} devices via cpal", cpal_devices.len());

                // Add cpal devices that aren't already in our list
                for cpal_device in cpal_devices {
                    if !all_devices
                        .iter()
                        .any(|existing| existing.name == cpal_device.name)
                    {
                        all_devices.push(cpal_device);
                    }
                }
            }
            Err(e) => {
                warn!("cpal enumeration failed: {}", e);
            }
        }

        crate::device_debug!("\n=== FINAL DEVICE LIST ===");
        for (i, device) in all_devices.iter().enumerate() {
            crate::device_debug!("  {}: {} ({})", i, device.name, device.id);
            crate::device_debug!(
                "     Input: {}, Output: {}, Default: {}",
                device.is_input,
                device.is_output,
                device.is_default
            );
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

    /// Fallback enumeration using cpal (existing method, renamed)
    pub async fn enumerate_cpal_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        crate::device_debug!("Starting cpal device enumeration...");

        // Debug: Check all available hosts and their devices
        crate::device_debug!("Available cpal hosts:");
        for host_id in cpal::ALL_HOSTS {
            crate::device_debug!("  - {:?}", host_id);
        }

        crate::device_debug!("Current host: {:?}", self.host.id());

        // Check what cpal can see vs what's actually available in the system
        #[cfg(target_os = "macos")]
        {
            crate::device_debug!("=== CPAL Device Detection Debug ===");

            // Check all hosts to see if any see more devices
            for host_id in cpal::ALL_HOSTS {
                match cpal::host_from_id(*host_id) {
                    Ok(host) => {
                        crate::device_debug!("Host {:?}:", host_id);

                        match host.output_devices() {
                            Ok(devices) => {
                                let devices_vec: Vec<_> = devices.collect();
                                crate::device_debug!(
                                    "  CPAL Output devices: {}",
                                    devices_vec.len()
                                );
                                for (i, device) in devices_vec.iter().enumerate() {
                                    let name =
                                        device.name().unwrap_or_else(|_| "Unknown".to_string());
                                    crate::device_debug!("    {}: {} {:?}", i, name, device.name());

                                    // Try to get device configs to see why some might be filtered
                                    match device.supported_output_configs() {
                                        Ok(configs) => {
                                            let config_count = configs.count();
                                            crate::device_debug!(
                                                "      -> {} supported configs",
                                                config_count
                                            );
                                        }
                                        Err(e) => {
                                            crate::device_debug!("      -> Config error: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                crate::device_debug!("  CPAL output devices error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        crate::device_debug!("  Host {:?} init failed: {}", host_id, e);
                    }
                }
            }

            // Let's try to access CoreAudio directly to see what's really available
            crate::device_debug!("\n=== System Profiler Shows These Devices ===");
            crate::device_debug!("- BenQ EW3270U (DisplayPort)");
            crate::device_debug!("- External Headphones (Built-in)");
            crate::device_debug!("- MacBook Pro Speakers (Built-in)");
            crate::device_debug!("- BlackHole 2ch (Virtual) - DEFAULT");
            crate::device_debug!("- Serato Virtual Audio (Virtual)");

            crate::device_debug!("\n=== Possible Issues ===");
            crate::device_debug!("1. cpal only shows 'active' devices");
            crate::device_debug!("2. Physical devices hidden when virtual device is default");
            crate::device_debug!("3. App permissions limiting device visibility");
            crate::device_debug!("4. CoreAudio routing configuration");
        }

        // **CRASH PREVENTION**: Use safe enumeration for both input and output devices
        crate::device_debug!("🛡️ Using crash-safe CPAL enumeration for input devices...");
        let input_devices_result = self.safe_enumerate_cpal_devices(true).await;

        crate::device_debug!("🛡️ Using crash-safe CPAL enumeration for output devices...");
        let output_devices_result = self.safe_enumerate_cpal_devices(false).await;

        let default_input = self.host.default_input_device();
        let default_output = self.host.default_output_device();

        crate::device_debug!(
            "Default input: {:?}",
            default_input.as_ref().and_then(|d| d.name().ok())
        );
        crate::device_debug!(
            "Default output: {:?}",
            default_output.as_ref().and_then(|d| d.name().ok())
        );

        let mut devices = Vec::new();
        let mut devices_map = HashMap::new();

        // Process input devices safely
        match input_devices_result {
            Ok(input_devices) => {
                crate::device_debug!("✅ Safely found {} input devices", input_devices.len());
                for (device, device_name) in input_devices {
                    let is_default = if let Some(ref default_device) = default_input {
                        device_name == default_device.name().unwrap_or_default()
                    } else {
                        false
                    };

                    if let Ok(device_info) = self.get_device_info(&device, true, false, is_default)
                    {
                        devices.push(device_info.clone());
                        devices_map.insert(device_info.id.clone(), device_info);
                    }
                }
            }
            Err(e) => {
                error!("⚠️ Failed to safely enumerate input devices: {}", e);
                error!("   Continuing with output device enumeration...");
            }
        }

        // Process output devices safely
        match output_devices_result {
            Ok(output_devices) => {
                crate::device_debug!("✅ Safely found {} output devices", output_devices.len());
                for (device, device_name) in output_devices {
                    crate::device_debug!("Processing output device: {}", device_name);

                    // Try to get more detailed device information
                    match device.supported_output_configs() {
                        Ok(configs) => {
                            crate::device_debug!("  Supported configs:");
                            for (i, config) in configs.enumerate() {
                                if i < 3 {
                                    // Limit output to prevent spam
                                    crate::device_debug!(
                                        "    - Sample rate: {}-{}, Channels: {}, Format: {:?}",
                                        config.min_sample_rate().0,
                                        config.max_sample_rate().0,
                                        config.channels(),
                                        config.sample_format()
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            crate::device_debug!("  Failed to get configs: {}", e);
                        }
                    }

                    let is_default = if let Some(ref default_device) = default_output {
                        device_name == default_device.name().unwrap_or_default()
                    } else {
                        false
                    };

                    match self.get_device_info(&device, false, true, is_default) {
                        Ok(device_info) => {
                            crate::device_debug!(
                                "Successfully added output device: {} (ID: {})",
                                device_info.name,
                                device_info.id
                            );
                            devices.push(device_info.clone());
                            devices_map.insert(device_info.id.clone(), device_info);
                        }
                        Err(e) => {
                            warn!("Failed to process output device '{}': {}", device_name, e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("⚠️ Failed to safely enumerate output devices: {}", e);
                error!("   This may prevent output device switching from working correctly.");
            }
        }

        // Add system default devices if they exist and weren't already detected
        #[cfg(target_os = "macos")]
        {
            self.add_system_defaults(
                &mut devices,
                &mut devices_map,
                &default_input,
                &default_output,
            )
            .await;
        }

        // Update cache by merging cpal devices (don't replace - preserve CoreAudio devices!)
        {
            let mut cache_guard = self.devices_cache.lock().await;
            for (device_id, device_info) in devices_map {
                cache_guard.insert(device_id, device_info);
            }
        }

        Ok(devices)
    }

    /// Add system default devices if they weren't already detected
    #[cfg(target_os = "macos")]
    async fn add_system_defaults(
        &self,
        devices: &mut Vec<AudioDeviceInfo>,
        devices_map: &mut HashMap<String, AudioDeviceInfo>,
        default_input: &Option<Device>,
        default_output: &Option<Device>,
    ) {
        // Only add actual system defaults if they exist
        if let Some(default_out) = default_output {
            let default_name = default_out
                .name()
                .unwrap_or_else(|_| "System Default Output".to_string());
            let default_id = "output_system_default";

            if !devices_map.contains_key(default_id) {
                crate::device_debug!("Adding system default output device: {}", default_name);
                if let Ok(default_device_info) =
                    self.get_device_info(default_out, false, true, true)
                {
                    let mut system_default = default_device_info;
                    system_default.id = default_id.to_string();
                    system_default.name = format!("{} (Default)", system_default.name);
                    devices.push(system_default.clone());
                    devices_map.insert(default_id.to_string(), system_default);
                }
            }
        }

        if let Some(default_in) = default_input {
            let default_name = default_in
                .name()
                .unwrap_or_else(|_| "System Default Input".to_string());
            let default_id = "input_system_default";

            if !devices_map.contains_key(default_id) {
                crate::device_debug!("Adding system default input device: {}", default_name);
                if let Ok(default_device_info) = self.get_device_info(default_in, true, false, true)
                {
                    let mut system_default = default_device_info;
                    system_default.id = default_id.to_string();
                    system_default.name = format!("{} (Default)", system_default.name);
                    devices.push(system_default.clone());
                    devices_map.insert(default_id.to_string(), system_default);
                }
            }
        }
    }

    /// Extract device information from a CPAL device
    fn get_device_info(
        &self,
        device: &Device,
        is_input: bool,
        is_output: bool,
        is_default: bool,
    ) -> Result<AudioDeviceInfo> {
        let name = device
            .name()
            .unwrap_or_else(|_| "Unknown Device".to_string());

        // Validate device name
        if name.len() > 512 {
            return Err(anyhow::anyhow!("Device name too long: {}", name.len()));
        }

        crate::device_debug!(
            "Processing device: {} (input: {}, output: {})",
            name,
            is_input,
            is_output
        );

        let id = format!(
            "{}_{}",
            if is_input { "input" } else { "output" },
            name.replace(" ", "_")
                .replace("(", "")
                .replace(")", "")
                .to_lowercase()
        );

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
                    for &rate in &SUPPORTED_INPUT_SAMPLE_RATES_HZ {
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
                    for &rate in &SUPPORTED_INPUT_SAMPLE_RATES_HZ {
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
            format!(
                "AirPods ({})",
                if cleaned.contains("Pro") {
                    "Pro"
                } else {
                    "Standard"
                }
            )
        } else {
            cleaned
        }
    }

    /// Safely enumerate CPAL devices with crash prevention
    pub async fn safe_enumerate_cpal_devices(
        &self,
        is_input: bool,
    ) -> Result<Vec<(Device, String)>> {
        crate::device_debug!(
            "🛡️  SAFE ENUMERATION: Starting crash-protected CPAL device enumeration"
        );

        // Use std::panic::catch_unwind to prevent CPAL crashes from killing the app
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if is_input {
                crate::device_debug!("🔍 SAFE ENUMERATION: Getting input devices iterator");
                let input_devices = self.host.input_devices()?;

                crate::device_debug!(
                    "🔍 SAFE ENUMERATION: Starting device iteration with individual error handling"
                );
                let mut devices = Vec::new();
                let mut device_count = 0;

                for device in input_devices {
                    crate::device_debug!(
                        "🔍 SAFE ENUMERATION: Processing input device #{}",
                        device_count
                    );

                    // Safely get device name with individual error handling
                    match device.name() {
                        Ok(name) => {
                            crate::device_debug!(
                                "✅ SAFE ENUMERATION: Successfully got input device name: {}",
                                name
                            );
                            devices.push((device, name));
                        }
                        Err(e) => {
                            crate::device_debug!("⚠️  SAFE ENUMERATION: Skipping input device #{} due to name access error: {}", device_count, e);
                            // Continue processing other devices instead of failing completely
                        }
                    }

                    device_count += 1;

                    // Safety limit to prevent infinite loops or excessive processing
                    if device_count > 50 {
                        crate::device_debug!(
                            "⚠️  SAFE ENUMERATION: Stopping after 50 input devices for safety"
                        );
                        break;
                    }
                }

                crate::device_debug!(
                    "✅ SAFE ENUMERATION: Successfully processed {} input devices",
                    devices.len()
                );
                Ok(devices)
            } else {
                crate::device_debug!("🔍 SAFE ENUMERATION: Getting output devices iterator");
                let output_devices = self.host.output_devices()?;

                crate::device_debug!("🔍 SAFE ENUMERATION: Starting output device iteration");
                let mut devices = Vec::new();
                let mut device_count = 0;

                for device in output_devices {
                    crate::device_debug!(
                        "🔍 SAFE ENUMERATION: Processing output device #{}",
                        device_count
                    );

                    // Safely get device name with individual error handling
                    match device.name() {
                        Ok(name) => {
                            crate::device_debug!(
                                "✅ SAFE ENUMERATION: Successfully got output device name: {}",
                                name
                            );
                            devices.push((device, name));
                        }
                        Err(e) => {
                            crate::device_debug!("⚠️  SAFE ENUMERATION: Skipping output device #{} due to name access error: {}", device_count, e);
                            // Continue processing other devices instead of failing completely
                        }
                    }

                    device_count += 1;

                    // Safety limit to prevent infinite loops or excessive processing
                    if device_count > 50 {
                        crate::device_debug!(
                            "⚠️  SAFE ENUMERATION: Stopping after 50 output devices for safety"
                        );
                        break;
                    }
                }

                crate::device_debug!(
                    "✅ SAFE ENUMERATION: Successfully processed {} output devices",
                    devices.len()
                );
                Ok(devices)
            }
        }));

        match result {
            Ok(device_result) => {
                match device_result {
                    Ok(devices) => {
                        crate::device_debug!("🎉 SAFE ENUMERATION: Successfully enumerated {} {} devices without crash",
                            devices.len(), if is_input { "input" } else { "output" });
                        Ok(devices)
                    }
                    Err(e) => {
                        error!("❌ SAFE ENUMERATION: CPAL enumeration failed: {}", e);
                        Err(e)
                    }
                }
            }
            Err(panic_payload) => {
                let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };

                error!(
                    "💥 PANIC CAUGHT: CPAL enumeration caused a panic: {}",
                    panic_msg
                );
                error!("   This prevents the app from crashing but indicates a serious CPAL/CoreAudio issue");

                Err(anyhow::anyhow!("CPAL enumeration panic: {}", panic_msg))
            }
        }
    }

    /// Get device cache reference
    pub fn get_devices_cache(&self) -> &Arc<Mutex<HashMap<String, AudioDeviceInfo>>> {
        &self.devices_cache
    }

    /// Get host reference
    pub fn get_host(&self) -> &Host {
        &self.host
    }

    /// Get CoreAudio integration reference
    pub fn get_coreaudio(&self) -> &CoreAudioIntegration {
        &self.coreaudio
    }
}

impl std::fmt::Debug for DeviceEnumerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceEnumerator")
            .field("host", &"Host(cpal)")
            .field("devices_cache", &self.devices_cache)
            .field("coreaudio", &"CoreAudioIntegration")
            .finish()
    }
}
