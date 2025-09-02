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
use tracing::{info, warn};

use crate::audio::types::AudioDeviceInfo;
use super::types::{DeviceHealth, DeviceStatus};

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

    /// Initialize device health tracking for a device
    pub async fn initialize_device_health(&self, device_info: &AudioDeviceInfo) {
        let mut health_guard = self.device_health.lock().await;
        
        let health = DeviceHealth::new_healthy(
            device_info.id.clone(), 
            device_info.name.clone()
        );
        
        health_guard.insert(device_info.id.clone(), health);
        crate::device_debug!("Initialized health tracking for device: {}", device_info.name);
    }

    /// Check if a device is still available and update its health status
    pub async fn check_device_health(&self, device_id: &str, device_exists: bool) -> Result<DeviceStatus> {
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
                        warn!("Device disconnected: {} (consecutive errors: {})", 
                            device_id, health.consecutive_errors);
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
            
            warn!("Device error for {}: {} (consecutive: {}, total: {})", 
                device_id, error, health.consecutive_errors, health.error_count);
        } else {
            // Create new health entry for unknown device
            let mut health = DeviceHealth::new_healthy(
                device_id.to_string(),
                format!("Unknown Device {}", device_id)
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
        let connected_devices = health_guard.values()
            .filter(|h| matches!(h.status, DeviceStatus::Connected))
            .count();
        let error_devices = health_guard.values()
            .filter(|h| matches!(h.status, DeviceStatus::Error(_)))
            .count();
        let avoided_devices = health_guard.values()
            .filter(|h| h.should_avoid())
            .count();
        
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

impl std::fmt::Debug for DeviceHealthMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceHealthMonitor")
            .field("device_health", &"HashMap<String, DeviceHealth>")
            .finish()
    }
}