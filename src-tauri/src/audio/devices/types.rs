// Core device types and enums for audio device management
//
// This module contains the fundamental data structures used throughout
// the audio device management system, including device status tracking,
// health monitoring, and error reporting.

/// Device connection status for error handling
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DeviceStatus {
    Connected,
    Disconnected,
    Error(String),
}

/// Device health information for monitoring
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceHealth {
    pub device_id: String,
    pub device_name: String,
    pub status: DeviceStatus,
    pub last_seen: u64, // Timestamp in milliseconds for serialization
    pub error_count: u32,
    pub consecutive_errors: u32,
}

impl DeviceHealth {
    /// Create a new healthy device health record
    pub fn new_healthy(device_id: String, device_name: String) -> Self {
        Self {
            device_id,
            device_name,
            status: DeviceStatus::Connected,
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            error_count: 0,
            consecutive_errors: 0,
        }
    }

    /// Update health status with a successful connection
    pub fn mark_connected(&mut self) {
        self.status = DeviceStatus::Connected;
        self.consecutive_errors = 0;
        self.update_last_seen();
    }

    /// Update health status with a disconnection
    pub fn mark_disconnected(&mut self) {
        self.status = DeviceStatus::Disconnected;
        self.consecutive_errors += 1;
        self.error_count += 1;
        self.update_last_seen();
    }

    /// Update health status with an error
    pub fn mark_error(&mut self, error: String) {
        self.status = DeviceStatus::Error(error);
        self.consecutive_errors += 1;
        self.error_count += 1;
        self.update_last_seen();
    }

    /// Check if device should be avoided due to consecutive errors
    pub fn should_avoid(&self) -> bool {
        self.consecutive_errors >= 3
    }

    /// Update the last seen timestamp
    fn update_last_seen(&mut self) {
        self.last_seen = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }
}