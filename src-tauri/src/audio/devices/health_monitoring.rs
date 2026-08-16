// Device health monitoring and error tracking system
//
// This module handles device health status, error counting, and availability
// tracking for audio devices. It provides functionality to monitor device
// stability and make intelligent decisions about device usage based on
// historical reliability.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

use super::types::{DeviceHealth, DeviceStatus};
use crate::audio::types::AudioDeviceInfo;

/// Device health monitoring system
pub struct DeviceHealthMonitor {
    device_health: Arc<Mutex<HashMap<String, DeviceHealth>>>,
}

impl DeviceHealthMonitor {
    /// Create a new health monitor
    pub fn new() -> Self {
        Self {
            device_health: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Begin tracking a device, keeping the history of one already known
    ///
    /// Enumeration runs continuously, so overwriting the record here would reset
    /// the error counts that `should_avoid` exists to accumulate.
    pub async fn initialize_device_health(&self, device_info: &AudioDeviceInfo) {
        let mut health_guard = self.device_health.lock().await;

        match health_guard.get_mut(&device_info.id) {
            Some(existing) => {
                existing.device_name = device_info.name.clone();
                existing.mark_present();
            }
            None => {
                let health =
                    DeviceHealth::new_healthy(device_info.id.clone(), device_info.name.clone());
                health_guard.insert(device_info.id.clone(), health);
                crate::device_debug!(
                    "Initialized health tracking for device: {}",
                    device_info.name
                );
            }
        }
    }

    /// Check if a device is still available and update its health status
    pub async fn check_device_health(
        &self,
        device_id: &str,
        device_exists: bool,
    ) -> Result<DeviceStatus> {
        let status = if device_exists {
            DeviceStatus::Connected
        } else {
            DeviceStatus::Disconnected
        };

        // Update device health tracking
        {
            let mut health_guard = self.device_health.lock().await;
            if let Some(health) = health_guard.get_mut(device_id) {
                match &status {
                    DeviceStatus::Connected => {
                        health.mark_connected();
                    }
                    DeviceStatus::Disconnected => {
                        health.mark_disconnected();
                        warn!(
                            "Device disconnected: {} (consecutive errors: {})",
                            device_id, health.consecutive_errors
                        );
                    }
                    DeviceStatus::Error(error) => {
                        health.mark_error(error.clone());
                    }
                }
            }
        }

        Ok(status)
    }

    /// Report a device error and update health tracking
    pub async fn report_device_error(&self, device_id: &str, error: String) {
        let mut health_guard = self.device_health.lock().await;

        if let Some(health) = health_guard.get_mut(device_id) {
            health.mark_error(error.clone());

            warn!(
                "Device error for {}: {} (consecutive: {}, total: {})",
                device_id, error, health.consecutive_errors, health.error_count
            );
        } else {
            // Create new health entry for unknown device
            let mut health = DeviceHealth::new_healthy(
                device_id.to_string(),
                format!("Unknown Device {}", device_id),
            );
            health.mark_error(error.clone());

            health_guard.insert(device_id.to_string(), health);
            warn!("New device error for {}: {}", device_id, error);
        }
    }

    /// Get device health information
    pub async fn get_device_health(&self, device_id: &str) -> Option<DeviceHealth> {
        let health_guard = self.device_health.lock().await;
        health_guard.get(device_id).cloned()
    }

    /// Get all device health information
    pub async fn get_all_device_health(&self) -> HashMap<String, DeviceHealth> {
        let health_guard = self.device_health.lock().await;
        health_guard.clone()
    }

    /// Check if a device should be avoided due to consecutive errors
    pub async fn should_avoid_device(&self, device_id: &str) -> bool {
        if let Some(health) = self.get_device_health(device_id).await {
            health.should_avoid()
        } else {
            false
        }
    }

    /// Get health statistics for monitoring
    pub async fn get_health_statistics(&self) -> HealthStatistics {
        let health_guard = self.device_health.lock().await;

        let total_devices = health_guard.len();
        let connected_devices = health_guard
            .values()
            .filter(|h| matches!(h.status, DeviceStatus::Connected))
            .count();
        let error_devices = health_guard
            .values()
            .filter(|h| matches!(h.status, DeviceStatus::Error(_)))
            .count();
        let avoided_devices = health_guard.values().filter(|h| h.should_avoid()).count();

        HealthStatistics {
            total_devices,
            connected_devices,
            error_devices,
            avoided_devices,
        }
    }
}

/// Health monitoring statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthStatistics {
    pub total_devices: usize,
    pub connected_devices: usize,
    pub error_devices: usize,
    pub avoided_devices: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str) -> AudioDeviceInfo {
        AudioDeviceInfo {
            id: id.to_string(),
            name: name.to_string(),
            uid: None,
            is_input: true,
            is_output: false,
            is_default: false,
            supported_sample_rates: vec![48000],
            supported_channels: vec![2],
            host_api: "CoreAudio".to_string(),
            transport: crate::audio::devices::transport::DeviceTransport::Usb,
        }
    }

    #[tokio::test]
    async fn enumeration_does_not_erase_recorded_errors() {
        let monitor = DeviceHealthMonitor::new();
        let device = device("mic-1", "Microphone");

        monitor.initialize_device_health(&device).await;
        monitor
            .report_device_error("mic-1", "stream failed to open".to_string())
            .await;

        // Enumeration runs constantly; it must not look like a clean slate
        monitor.initialize_device_health(&device).await;

        let health = monitor.get_device_health("mic-1").await.unwrap();
        assert_eq!(health.consecutive_errors, 1);
        assert_eq!(health.error_count, 1);
        assert!(matches!(health.status, DeviceStatus::Error(_)));
    }

    #[tokio::test]
    async fn repeated_errors_eventually_flag_a_device_to_avoid() {
        let monitor = DeviceHealthMonitor::new();
        let device = device("mic-1", "Microphone");
        monitor.initialize_device_health(&device).await;

        for attempt in 0..3 {
            monitor
                .report_device_error("mic-1", format!("failure {}", attempt))
                .await;
            // Each retry re-enumerates before touching the device again
            monitor.initialize_device_health(&device).await;
        }

        assert!(monitor.should_avoid_device("mic-1").await);
        assert_eq!(
            monitor
                .get_device_health("mic-1")
                .await
                .unwrap()
                .error_count,
            3
        );
    }

    #[tokio::test]
    async fn a_successful_check_clears_the_error_streak() {
        let monitor = DeviceHealthMonitor::new();
        let device = device("mic-1", "Microphone");
        monitor.initialize_device_health(&device).await;

        for _ in 0..3 {
            monitor
                .report_device_error("mic-1", "failure".to_string())
                .await;
        }
        assert!(monitor.should_avoid_device("mic-1").await);

        monitor.check_device_health("mic-1", true).await.unwrap();

        let health = monitor.get_device_health("mic-1").await.unwrap();
        assert_eq!(health.consecutive_errors, 0, "streak resets");
        assert_eq!(health.error_count, 3, "lifetime total is kept");
        assert!(!monitor.should_avoid_device("mic-1").await);
    }

    #[tokio::test]
    async fn reappearing_after_a_disconnect_marks_the_device_connected() {
        let monitor = DeviceHealthMonitor::new();
        let device = device("mic-1", "Microphone");
        monitor.initialize_device_health(&device).await;

        monitor.check_device_health("mic-1", false).await.unwrap();
        let disconnected = monitor.get_device_health("mic-1").await.unwrap();
        assert!(matches!(disconnected.status, DeviceStatus::Disconnected));

        monitor.initialize_device_health(&device).await;

        let health = monitor.get_device_health("mic-1").await.unwrap();
        assert!(matches!(health.status, DeviceStatus::Connected));
        assert_eq!(health.error_count, 1, "the disconnect is still on record");
    }

    #[tokio::test]
    async fn last_seen_tracks_presence_rather_than_the_latest_update() {
        let monitor = DeviceHealthMonitor::new();
        let device = device("mic-1", "Microphone");
        monitor.initialize_device_health(&device).await;

        let seen_while_present = monitor.get_device_health("mic-1").await.unwrap().last_seen;

        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        monitor
            .report_device_error("mic-1", "failure".to_string())
            .await;

        assert_eq!(
            monitor.get_device_health("mic-1").await.unwrap().last_seen,
            seen_while_present,
            "an error is not a sighting"
        );
    }

    #[tokio::test]
    async fn a_renamed_device_keeps_its_history_under_the_same_id() {
        let monitor = DeviceHealthMonitor::new();
        monitor
            .initialize_device_health(&device("mic-1", "Microphone"))
            .await;
        monitor
            .report_device_error("mic-1", "failure".to_string())
            .await;

        monitor
            .initialize_device_health(&device("mic-1", "Studio Microphone"))
            .await;

        let health = monitor.get_device_health("mic-1").await.unwrap();
        assert_eq!(health.device_name, "Studio Microphone");
        assert_eq!(health.consecutive_errors, 1);
    }

    #[tokio::test]
    async fn health_statistics_count_each_device_once() {
        let monitor = DeviceHealthMonitor::new();
        monitor
            .initialize_device_health(&device("mic-1", "Microphone"))
            .await;
        monitor
            .initialize_device_health(&device("out-1", "Speakers"))
            .await;
        // A second enumeration pass must not double count
        monitor
            .initialize_device_health(&device("mic-1", "Microphone"))
            .await;

        for _ in 0..3 {
            monitor
                .report_device_error("mic-1", "failure".to_string())
                .await;
        }

        let stats = monitor.get_health_statistics().await;
        assert_eq!(stats.total_devices, 2);
        assert_eq!(stats.connected_devices, 1);
        assert_eq!(stats.error_devices, 1);
        assert_eq!(stats.avoided_devices, 1);
    }

    #[tokio::test]
    async fn an_untracked_device_is_not_avoided() {
        let monitor = DeviceHealthMonitor::new();
        assert!(!monitor.should_avoid_device("never-seen").await);
        assert!(monitor.get_device_health("never-seen").await.is_none());
        assert!(monitor.get_all_device_health().await.is_empty());
    }
}

impl std::fmt::Debug for DeviceHealthMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceHealthMonitor")
            .field("device_health", &"HashMap<String, DeviceHealth>")
            .finish()
    }
}
