// Audio devices module - Device management and hardware interfacing
//
// This module provides comprehensive audio device management through
// a modular architecture with clear separation of concerns:
// - types: Core device types and health structures
// - enumeration: Device discovery CoreAudio enumeration
// - coreaudio_integration: Platform-specific CoreAudio functionality
// - health_monitoring: Device reliability tracking and error management
// - device_manager: High-level public API and orchestration
// - coreaudio_stream: Platform-specific streaming (existing, macOS only)

// Core modules for device management
pub mod coreaudio_integration;
pub mod device_manager;
pub mod enumeration;
pub mod health_monitoring;
pub mod transport;
pub mod types;

#[cfg(target_os = "macos")]
pub mod coreaudio_stream;
#[cfg(target_os = "macos")]
pub mod device_watcher;
#[cfg(target_os = "macos")]
pub mod property_listener;
#[cfg(target_os = "macos")]
pub mod system_audio_router;
#[cfg(target_os = "macos")]
pub mod virtual_driver;

pub use device_manager::AudioDeviceManager;

pub use types::{DeviceHealth, DeviceStatus};

pub use health_monitoring::{DeviceHealthMonitor, HealthStatistics};

#[cfg(target_os = "macos")]
pub use coreaudio_stream::{CoreAudioInputStream, CoreAudioOutputStream};
#[cfg(target_os = "macos")]
pub use device_watcher::{
    DeviceWatcher, DEVICES_CHANGED_EVENT, DEVICE_DISCONNECTED_EVENT, DEVICE_RECONNECTED_EVENT,
};
#[cfg(target_os = "macos")]
pub use property_listener::{DeviceEvent, DevicePropertyListener};
#[cfg(target_os = "macos")]
pub use system_audio_router::{DiversionOutcome, SystemAudioRouter};
#[cfg(target_os = "macos")]
pub use virtual_driver::VirtualDriverManager;
