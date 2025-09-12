// High-level device management and public API
//
// This module provides the main AudioDeviceManager struct and coordinates
// between the enumeration system, health monitoring, and platform-specific
// integrations. It serves as the primary interface for device operations
// throughout the application.

use anyhow::Result;
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

        Err(anyhow::anyhow!(
            "Device not found after refresh: {}",
            device_id
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
