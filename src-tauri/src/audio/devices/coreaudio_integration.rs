// Platform-specific CoreAudio integration for macOS
//
// This module provides direct CoreAudio API integration for device discovery,
// device ID resolution, and default device checking. It handles the low-level
// CoreAudio system calls and provides a clean interface for the higher-level
// device management system.

#[cfg(target_os = "macos")]
use anyhow::Result;
#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tokio::sync::Mutex;
#[cfg(target_os = "macos")]
use tracing::{info, warn};

#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(target_os = "macos")]
use coreaudio_sys::{
    kAudioDevicePropertyDeviceNameCFString, kAudioDevicePropertyStreams,
    kAudioHardwarePropertyDefaultInputDevice, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMaster,
    kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyScopeInput,
    kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject, AudioDeviceID,
    AudioObjectPropertyAddress,
};

#[cfg(target_os = "macos")]
use crate::audio::types::{AudioDeviceHandle, AudioDeviceInfo, CoreAudioDevice};

/// CoreAudio integration manager for direct system audio access
#[cfg(target_os = "macos")]
pub struct CoreAudioIntegration {
    devices_cache: Arc<Mutex<HashMap<String, AudioDeviceInfo>>>,
}

#[cfg(target_os = "macos")]
impl CoreAudioIntegration {
    /// Create a new CoreAudio integration manager
    pub fn new(devices_cache: Arc<Mutex<HashMap<String, AudioDeviceInfo>>>) -> Self {
        Self { devices_cache }
    }

    /// Enumerate devices using direct CoreAudio API access
    pub async fn enumerate_coreaudio_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        use std::mem;
        use std::ptr;

        let mut devices = Vec::new();

        info!("Starting CoreAudio direct device enumeration");

        // Get all audio devices from CoreAudio
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        // Get the number of devices
        let mut data_size: u32 = 0;
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyDataSize(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut data_size as *mut _,
            )
        };

        if status != 0 {
            return Err(anyhow::anyhow!(
                "Failed to get CoreAudio device count: {}",
                status
            ));
        }

        let device_count = data_size / mem::size_of::<AudioDeviceID>() as u32;
        crate::device_debug!("CoreAudio reports {} total audio devices", device_count);

        if device_count == 0 {
            return Ok(devices);
        }

        // Get the device IDs
        let mut device_ids: Vec<AudioDeviceID> = vec![0; device_count as usize];
        let mut actual_size = data_size;

        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut actual_size as *mut _,
                device_ids.as_mut_ptr() as *mut _,
            )
        };

        if status != 0 {
            return Err(anyhow::anyhow!(
                "Failed to get CoreAudio device IDs: {}",
                status
            ));
        }

        // Process each device
        for device_id in device_ids {
            match self.get_coreaudio_device_info(device_id).await {
                Ok(device_infos) => {
                    for device_info in device_infos {
                        crate::device_debug!("Found CoreAudio device: {} ({})", device_info.name, device_info.id);
                        devices.push(device_info);
                    }
                }
                Err(e) => {
                    warn!("Failed to get info for device {}: {}", device_id, e);
                }
            }
        }

        Ok(devices)
    }

    /// Get device info from CoreAudio device ID - returns multiple entries for dual-capability devices
    async fn get_coreaudio_device_info(
        &self,
        device_id: AudioDeviceID,
    ) -> Result<Vec<AudioDeviceInfo>> {
        use std::mem;
        use std::ptr;

        // Get device name and convert to String immediately to avoid Send issues
        let device_name = {
            let name_property = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyDeviceNameCFString,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMaster,
            };

            let mut name_size = mem::size_of::<CFStringRef>() as u32;
            let mut cf_string_ref: CFStringRef = ptr::null();

            let status = unsafe {
                coreaudio_sys::AudioObjectGetPropertyData(
                    device_id,
                    &name_property as *const _,
                    0,
                    ptr::null(),
                    &mut name_size as *mut _,
                    &mut cf_string_ref as *mut _ as *mut _,
                )
            };

            if status != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to get device name for device {}: {}",
                    device_id,
                    status
                ));
            }

            let cf_string = unsafe { CFString::wrap_under_get_rule(cf_string_ref) };
            cf_string.to_string()
        };

        // Check if device has output streams
        let has_output = self.device_has_streams(device_id, false).await?;

        // Check if device has input streams
        let has_input = self.device_has_streams(device_id, true).await?;

        // Skip devices that have neither input nor output
        if !has_input && !has_output {
            return Ok(Vec::new());
        }

        let clean_name = device_name
            .replace(" ", "_")
            .replace("(", "")
            .replace(")", "")
            .to_lowercase();

        // For devices that support both input and output, we need to create separate entries
        let mut device_infos = Vec::new();

        // Create input device entry if device supports input
        if has_input {
            let input_device_id = format!("input_{}", clean_name);
            let is_default_input = self
                .is_coreaudio_default_device(device_id, false)
                .await
                .unwrap_or(false);

            device_infos.push(AudioDeviceInfo {
                id: input_device_id,
                name: device_name.clone(),
                is_input: true,
                is_output: false,
                is_default: is_default_input,
                supported_sample_rates: vec![48000, 44100], // Prioritize 48kHz to match system default
                supported_channels: vec![2],                // Assume stereo
                host_api: "CoreAudio (Direct)".to_string(),
            });
        }

        // Create output device entry if device supports output
        if has_output {
            let output_device_id = format!("output_{}", clean_name);
            let is_default_output = self
                .is_coreaudio_default_device(device_id, true)
                .await
                .unwrap_or(false);

            device_infos.push(AudioDeviceInfo {
                id: output_device_id,
                name: device_name.clone(),
                is_input: false,
                is_output: true,
                is_default: is_default_output,
                supported_sample_rates: vec![48000, 44100], // Prioritize 48kHz to match system default
                supported_channels: vec![2],                // Assume stereo
                host_api: "CoreAudio (Direct)".to_string(),
            });
        }

        Ok(device_infos)
    }

    /// Check if a device has streams in the specified direction
    async fn device_has_streams(&self, device_id: AudioDeviceID, is_input: bool) -> Result<bool> {
        use std::ptr;

        let streams_property = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyStreams,
            mScope: if is_input {
                kAudioObjectPropertyScopeInput
            } else {
                kAudioObjectPropertyScopeOutput
            },
            mElement: kAudioObjectPropertyElementMaster,
        };

        let mut streams_size: u32 = 0;
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyDataSize(
                device_id,
                &streams_property as *const _,
                0,
                ptr::null(),
                &mut streams_size as *mut _,
            )
        };

        Ok(status == 0 && streams_size > 0)
    }

    /// Check if a CoreAudio device is the system default
    pub async fn is_coreaudio_default_device(
        &self,
        device_id: AudioDeviceID,
        is_output: bool,
    ) -> Result<bool> {
        use std::mem;
        use std::ptr;

        let property_selector = if is_output {
            kAudioHardwarePropertyDefaultOutputDevice
        } else {
            kAudioHardwarePropertyDefaultInputDevice
        };

        let property = AudioObjectPropertyAddress {
            mSelector: property_selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        let mut default_device_id: AudioDeviceID = 0;
        let mut size = mem::size_of::<AudioDeviceID>() as u32;

        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &property as *const _,
                0,
                ptr::null(),
                &mut size as *mut _,
                &mut default_device_id as *mut _ as *mut _,
            )
        };

        if status == 0 {
            Ok(default_device_id == device_id)
        } else {
            Ok(false)
        }
    }

    /// Find the CoreAudio device ID by name
    pub async fn find_coreaudio_device_id(&self, device_name: &str) -> Result<AudioDeviceID> {
        use std::mem;
        use std::ptr;

        // Get all audio devices from CoreAudio
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        // Get device count
        let mut data_size: u32 = 0;
        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyDataSize(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut data_size as *mut _,
            )
        };

        if status != 0 {
            return Err(anyhow::anyhow!(
                "Failed to get CoreAudio device count: {}",
                status
            ));
        }

        let device_count = data_size / mem::size_of::<AudioDeviceID>() as u32;
        let mut device_ids: Vec<AudioDeviceID> = vec![0; device_count as usize];
        let mut actual_size = data_size;

        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &property_address as *const _,
                0,
                ptr::null(),
                &mut actual_size as *mut _,
                device_ids.as_mut_ptr() as *mut _,
            )
        };

        if status != 0 {
            return Err(anyhow::anyhow!(
                "Failed to get CoreAudio device IDs: {}",
                status
            ));
        }

        // Find device by name
        for device_id in device_ids {
            if let Ok(name) = self.get_coreaudio_device_name(device_id).await {
                if name == device_name {
                    return Ok(device_id);
                }
            }
        }

        Err(anyhow::anyhow!(
            "CoreAudio device not found: {}",
            device_name
        ))
    }

    /// Get CoreAudio device name by ID
    pub async fn get_coreaudio_device_name(&self, device_id: AudioDeviceID) -> Result<String> {
        use std::mem;
        use std::ptr;

        let name_property = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyDeviceNameCFString,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        let mut name_size = mem::size_of::<CFStringRef>() as u32;
        let mut cf_string_ref: CFStringRef = ptr::null();

        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                device_id,
                &name_property as *const _,
                0,
                ptr::null(),
                &mut name_size as *mut _,
                &mut cf_string_ref as *mut _ as *mut _,
            )
        };

        if status != 0 {
            return Err(anyhow::anyhow!(
                "Failed to get device name for device {}: {}",
                device_id,
                status
            ));
        }

        let cf_string = unsafe { CFString::wrap_under_get_rule(cf_string_ref) };
        Ok(cf_string.to_string())
    }

    /// Create a CoreAudio device handle for direct audio streaming
    pub async fn create_coreaudio_device_handle(
        &self,
        device_info: &AudioDeviceInfo,
        _is_input: bool,
    ) -> Result<AudioDeviceHandle> {
        // Extract the actual CoreAudio device ID from our device info
        // We need to re-enumerate to get the raw device ID
        match self.find_coreaudio_device_id(&device_info.name).await {
            Ok(device_id) => {
                info!(
                    "Creating CoreAudio handle for device {} (ID: {})",
                    device_info.name, device_id
                );
                Ok(AudioDeviceHandle::CoreAudio(CoreAudioDevice {
                    device_id,
                    name: device_info.name.clone(),
                    sample_rate: 48000, // Match system default (was 44100)
                    channels: 2,        // Default stereo
                    stream: None,       // Stream will be created when needed
                }))
            }
            Err(e) => {
                warn!(
                    "Failed to find CoreAudio device ID for {}: {}",
                    device_info.name, e
                );
                Err(e)
            }
        }
    }
}

// Non-macOS stub implementations
#[cfg(not(target_os = "macos"))]
pub struct CoreAudioIntegration;

#[cfg(not(target_os = "macos"))]
impl CoreAudioIntegration {
    pub fn new(
        _devices_cache: std::sync::Arc<
            tokio::sync::Mutex<
                std::collections::HashMap<String, crate::audio::types::AudioDeviceInfo>,
            >,
        >,
    ) -> Self {
        Self
    }

    pub async fn enumerate_coreaudio_devices(
        &self,
    ) -> anyhow::Result<Vec<crate::audio::types::AudioDeviceInfo>> {
        Ok(Vec::new())
    }

    pub async fn create_coreaudio_device_handle(
        &self,
        _device_info: &crate::audio::types::AudioDeviceInfo,
        _is_input: bool,
    ) -> anyhow::Result<crate::audio::types::AudioDeviceHandle> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
    }
}
