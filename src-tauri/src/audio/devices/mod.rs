// Audio devices module - Device management and hardware interfacing
//
// This module provides comprehensive audio device management through
// a modular architecture with clear separation of concerns:
// - types: Core device types and health structures
// - enumeration: Device discovery and CPAL/CoreAudio enumeration
// - coreaudio_integration: Platform-specific CoreAudio functionality
// - health_monitoring: Device reliability tracking and error management
// - device_manager: High-level public API and orchestration
// - monitor: Device monitoring and health tracking (existing)
// - coreaudio_stream: Platform-specific streaming (existing, macOS only)

// Core modules for device management
pub mod coreaudio_integration;
pub mod device_manager;
pub mod enumeration;
pub mod health_monitoring;
pub mod types;

// Existing modules (preserved)
pub mod monitor;

#[cfg(target_os = "macos")]
pub mod coreaudio_stream;

// CoreAudio input stream as alternative to CPAL (not a full replacement)

// Re-export main public API
pub use device_manager::AudioDeviceManager;

// Re-export core types
pub use types::{DeviceHealth, DeviceStatus};

// Re-export health monitoring functionality
pub use health_monitoring::{DeviceHealthMonitor, HealthStatistics};

// Re-export existing monitor types (preserved for backward compatibility)
pub use monitor::{
    get_device_monitor, get_device_monitoring_stats, DeviceMonitor, DeviceMonitorConfig,
    DeviceMonitorStats,
};

// Re-export existing CoreAudio streams (preserved for backward compatibility)
#[cfg(target_os = "macos")]
pub use coreaudio_stream::{CoreAudioInputStream, CoreAudioOutputStream};

// CoreAudio input streams work as CPAL alternatives, using existing audio pipeline
