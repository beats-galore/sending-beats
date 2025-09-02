// High-level device management and public API
//
// This module provides the main AudioDeviceManager struct and coordinates
// between the enumeration system, health monitoring, and platform-specific
// integrations. It serves as the primary interface for device operations
// throughout the application.

use anyhow::Result;
use cpal::Device;
use std::collections::HashMap;
use tracing::{error, info, warn};

use super::enumeration::DeviceEnumerator;
use super::health_monitoring::DeviceHealthMonitor;
use super::types::{DeviceHealth, DeviceStatus};
use crate::audio::types::{AudioDeviceHandle, AudioDeviceInfo};

/// Cross-platform audio device manager with health monitoring and caching
pub struct AudioDeviceManager {
    enumerator: DeviceEnumerator,
    health_monitor: DeviceHealthMonitor,
}

impl std::fmt::Debug for AudioDeviceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioDeviceManager")
            .field("enumerator", &self.enumerator)
            .field("health_monitor", &"DeviceHealthMonitor")
            .finish()
    }
}

impl AudioDeviceManager {
    /// Create a new audio device manager
    pub fn new() -> Result<Self> {
        let enumerator = DeviceEnumerator::new()?;
        let health_monitor = DeviceHealthMonitor::new();

        Ok(Self {
            enumerator,
            health_monitor,
        })
    }

    /// Get ALL audio devices including hardware bypassed by system routing
    pub async fn enumerate_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        let devices = self.enumerator.enumerate_devices().await?;

        // Initialize health tracking for new devices
        for device in &devices {
            self.health_monitor.initialize_device_health(device).await;
        }

        Ok(devices)
    }

    /// Get device by ID from cache
    pub async fn get_device(&self, device_id: &str) -> Option<AudioDeviceInfo> {
        let cache = self.enumerator.get_devices_cache();
        cache.lock().await.get(device_id).cloned()
    }

    /// Force refresh device list
    pub async fn refresh_devices(&self) -> Result<()> {
        let _devices = self.enumerate_devices().await?;
        Ok(())
    }

    /// Check if a device is still available and update its health status
    pub async fn check_device_health(&self, device_id: &str) -> Result<DeviceStatus> {
        // Try to enumerate devices to see if the device still exists
        let devices = self.enumerate_devices().await?;
        let device_exists = devices.iter().any(|d| d.id == device_id);

        self.health_monitor
            .check_device_health(device_id, device_exists)
            .await
    }

    /// Report a device error and update health tracking
    pub async fn report_device_error(&self, device_id: &str, error: String) {
        self.health_monitor
            .report_device_error(device_id, error)
            .await;
    }

    /// Get device health information
    pub async fn get_device_health(&self, device_id: &str) -> Option<DeviceHealth> {
        self.health_monitor.get_device_health(device_id).await
    }

    /// Get all device health information
    pub async fn get_all_device_health(&self) -> HashMap<String, DeviceHealth> {
        self.health_monitor.get_all_device_health().await
    }

    /// Check if a device should be avoided due to consecutive errors
    pub async fn should_avoid_device(&self, device_id: &str) -> bool {
        self.health_monitor.should_avoid_device(device_id).await
    }

    /// Find audio device for streaming - tries CoreAudio first, then cpal fallback
    pub async fn find_audio_device(
        &self,
        device_id: &str,
        is_input: bool,
    ) -> Result<AudioDeviceHandle> {
        info!(
            "Searching for audio device: {} (input: {})",
            device_id, is_input
        );

        // First try to find the device in our cache
        if let Some(device_info) = self.get_device(device_id).await {
            if device_info.host_api == "CoreAudio (Direct)" {
                crate::device_debug!("Found CoreAudio device: {}", device_info.name);
                return self
                    .enumerator
                    .get_coreaudio()
                    .create_coreaudio_device_handle(&device_info, is_input)
                    .await;
            }
        }

        // If not found in cache, refresh the device list and try again
        info!("Device not found in cache, refreshing device list...");
        let _refreshed_devices = self.enumerate_devices().await?;

        if let Some(device_info) = self.get_device(device_id).await {
            if device_info.host_api == "CoreAudio (Direct)" {
                crate::device_debug!("Found CoreAudio device after refresh: {}", device_info.name);
                return self
                    .enumerator
                    .get_coreaudio()
                    .create_coreaudio_device_handle(&device_info, is_input)
                    .await;
            }
        }

        // Fallback to cpal device search
        match self.find_cpal_device(device_id, is_input).await {
            Ok(cpal_device) => {
                info!("Found cpal device: {}", device_id);
                Ok(AudioDeviceHandle::Cpal(cpal_device))
            }
            Err(e) => {
                warn!("No device found for '{}': {}", device_id, e);
                Err(e)
            }
        }
    }

    /// Find actual cpal Device by device_id for real audio I/O with crash prevention
    pub async fn find_cpal_device(&self, device_id: &str, is_input: bool) -> Result<Device> {
        info!(
            "🔍 CPAL DEVICE SEARCH: Looking for {} device: {}",
            if is_input { "input" } else { "output" },
            device_id
        );

        // First check if this is a known device from our cache
        let device_info = self.get_device(device_id).await;
        if let Some(info) = device_info {
            info!(
                "📋 CACHE HIT: Found device info: {} -> {}",
                device_id, info.name
            );
        }

        // Use CPAL-safe device enumeration that prevents crashes
        match self.enumerator.safe_enumerate_cpal_devices(is_input).await {
            Ok(devices) => {
                crate::audio_debug!(
                    "✅ SAFE ENUMERATION: Found {} {} devices",
                    devices.len(),
                    if is_input { "input" } else { "output" }
                );

                // Search through the safely enumerated devices
                for (index, (device, device_name)) in devices.into_iter().enumerate() {
                    let generated_id = format!(
                        "{}_{}",
                        if is_input { "input" } else { "output" },
                        device_name
                            .replace(" ", "_")
                            .replace("(", "")
                            .replace(")", "")
                            .to_lowercase()
                    );

                    crate::audio_debug!(
                        "🔍 DEVICE CHECK [{}]: '{}' -> '{}'",
                        index,
                        device_name,
                        generated_id
                    );

                    if generated_id == device_id {
                        crate::audio_debug!(
                            "✅ DEVICE MATCH: Found {} device: {}",
                            if is_input { "input" } else { "output" },
                            device_name
                        );
                        return Ok(device);
                    }
                }

                // No exact device match found
                return Err(anyhow::anyhow!("Device not found: {}", device_id));
            }
            Err(e) => {
                error!(
                    "❌ ENUMERATION FAILED: CPAL device enumeration crashed: {}",
                    e
                );
                error!("   Device: {}, Input: {}", device_id, is_input);
                error!("   This indicates a deeper CPAL/CoreAudio issue");

                // Try system default as absolute fallback
                if device_id.contains("default") {
                    info!("🆘 LAST RESORT: Attempting system default device");
                    return self.get_system_default_device(is_input).await;
                }

                return Err(anyhow::anyhow!(
                    "CPAL enumeration failure: Cannot access {} device '{}' due to system crash. Error: {}",
                    if is_input { "input" } else { "output" }, device_id, e
                ));
            }
        }
    }

    /// Get system default device as absolute fallback
    async fn get_system_default_device(&self, is_input: bool) -> Result<Device> {
        info!(
            "🆘 SYSTEM DEFAULT: Attempting to use system default {} device",
            if is_input { "input" } else { "output" }
        );

        use cpal::traits::HostTrait;

        let host = self.enumerator.get_host();

        if is_input {
            if let Some(default_device) = host.default_input_device() {
                info!("✅ SYSTEM DEFAULT: Using system default input device");
                return Ok(default_device);
            }
        } else {
            if let Some(default_device) = host.default_output_device() {
                info!("✅ SYSTEM DEFAULT: Using system default output device");
                return Ok(default_device);
            }
        }

        Err(anyhow::anyhow!(
            "No system default {} device available",
            if is_input { "input" } else { "output" }
        ))
    }

    /// Initialize device health tracking for a device
    pub async fn initialize_device_health(&self, device_info: &AudioDeviceInfo) {
        self.health_monitor
            .initialize_device_health(device_info)
            .await;
    }

    /// Get health monitoring statistics
    pub async fn get_health_statistics(&self) -> super::health_monitoring::HealthStatistics {
        self.health_monitor.get_health_statistics().await
    }
}
