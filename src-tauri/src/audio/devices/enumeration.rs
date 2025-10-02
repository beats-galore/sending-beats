// Device discovery and enumeration system
//
// This module handles the core device enumeration logic, safe device discovery with crash protection.
// It provides device information extraction, name cleaning, and availability
// filtering.

use super::coreaudio_integration::CoreAudioIntegration;
use crate::audio::types::AudioDeviceInfo;
use crate::types::SUPPORTED_INPUT_SAMPLE_RATES_HZ;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// Device enumeration system with crash protection
pub struct DeviceEnumerator {
    devices_cache: Arc<Mutex<HashMap<String, AudioDeviceInfo>>>,
    coreaudio: CoreAudioIntegration,
}

impl DeviceEnumerator {
    /// Create a new device enumerator
    pub fn new() -> Result<Self> {
        crate::device_debug!("Initializing CoreAudio-only device enumerator");

        let devices_cache = Arc::new(Mutex::new(HashMap::new()));
        let coreaudio = CoreAudioIntegration::new(devices_cache.clone());

        Ok(Self {
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
                    error!("CoreAudio direct access failed: {}", e);
                }
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

        // Update cache with all devices
        {
            let mut cache_guard = self.devices_cache.lock().await;
            for device in &all_devices {
                cache_guard.insert(device.id.clone(), device.clone());
            }
        }

        Ok(all_devices)
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

    /// Get device cache reference
    pub fn get_devices_cache(&self) -> &Arc<Mutex<HashMap<String, AudioDeviceInfo>>> {
        &self.devices_cache
    }

    /// Get CoreAudio integration reference
    pub fn get_coreaudio(&self) -> &CoreAudioIntegration {
        &self.coreaudio
    }
}

impl std::fmt::Debug for DeviceEnumerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceEnumerator")
            .field("devices_cache", &self.devices_cache)
            .field("coreaudio", &"CoreAudioIntegration")
            .finish()
    }
}
