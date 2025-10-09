// Audio devices module - Device management and hardware interfacing
//
// This module provides comprehensive audio device management through
// a modular architecture with clear separation of concerns:
// - types: Core device types and health structures
// - enumeration: Device discovery CoreAudio enumeration
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
pub mod aggregate_device;
#[cfg(target_os = "macos")]
pub mod coreaudio_stream;
#[cfg(target_os = "macos")]
pub mod device_hog;
#[cfg(target_os = "macos")]
pub mod system_audio_router;

pub use device_manager::AudioDeviceManager;

pub use types::{DeviceHealth, DeviceStatus};

pub use health_monitoring::{DeviceHealthMonitor, HealthStatistics};

pub use monitor::{
    get_device_monitor, get_device_monitoring_stats, DeviceMonitor, DeviceMonitorConfig,
    DeviceMonitorStats,
};

#[cfg(target_os = "macos")]
pub use aggregate_device::AggregateDeviceManager;
#[cfg(target_os = "macos")]
pub use coreaudio_stream::{CoreAudioInputStream, CoreAudioOutputStream};
#[cfg(target_os = "macos")]
pub use device_hog::DeviceHogManager;
#[cfg(target_os = "macos")]
pub use system_audio_router::SystemAudioRouter;
