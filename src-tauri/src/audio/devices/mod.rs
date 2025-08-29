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
pub mod types;
pub mod enumeration;
pub mod coreaudio_integration;
pub mod health_monitoring;
pub mod device_manager;

// Existing modules (preserved)
pub mod monitor;

#[cfg(target_os = "macos")]
pub mod coreaudio_stream;

// Re-export main public API
pub use device_manager::AudioDeviceManager;

// Re-export core types
pub use types::{DeviceStatus, DeviceHealth};

// Re-export health monitoring functionality
pub use health_monitoring::{DeviceHealthMonitor, HealthStatistics};

// Re-export existing monitor types (preserved for backward compatibility)
pub use monitor::{
    DeviceMonitor, DeviceMonitorConfig, DeviceMonitorStats,
    initialize_device_monitoring, get_device_monitor, stop_device_monitoring, get_device_monitoring_stats,
};

// Re-export existing CoreAudio stream (preserved for backward compatibility)
#[cfg(target_os = "macos")]
pub use coreaudio_stream::CoreAudioOutputStream;