use crate::audio::tap::core_audio_bindings::{
    AudioObjectGetPropertyData, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectSetPropertyData, OSStatus,
};
use anyhow::Result;
use colored::Colorize;
use std::ffi::c_void;
use tracing::{error, info, warn};

const KAUDIO_DEVICE_PROPERTY_HOG_MODE: u32 = 1869636203; // 'oink' (0x6F696E6B)
const KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = 1735159650; // 'glob' (0x676C6F62)
const KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER: u32 = 0; // 0x00000000

/// Device hog mode manager for exclusive audio device access
pub struct DeviceHogManager;

impl DeviceHogManager {
    /// Take exclusive control (hog) of an audio device
    /// Returns true if successfully hogged, false if already hogged by another process
    pub fn hog_device(device_id: AudioObjectID) -> Result<bool> {
        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_DEVICE_PROPERTY_HOG_MODE,
                mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
            };

            // Request exclusive access by setting hog_pid to -1
            let mut hog_pid: i32 = -1;
            let size = std::mem::size_of::<i32>() as u32;

            let status: OSStatus = AudioObjectSetPropertyData(
                device_id,
                &address,
                0,
                std::ptr::null(),
                size,
                &mut hog_pid as *mut i32 as *mut c_void,
            );

            if status != 0 {
                error!(
                    "{} Failed to hog device {}: OSStatus {}",
                    "HOG_ERROR".on_red().white(),
                    device_id,
                    status
                );
                return Err(anyhow::anyhow!(
                    "Failed to hog device {}: OSStatus {}",
                    device_id,
                    status
                ));
            }

            // Check if we successfully acquired the hog
            let current_pid = std::process::id() as i32;
            if hog_pid == current_pid {
                info!(
                    "{} Successfully hogged device {} (PID: {})",
                    "HOG_ACQUIRED".on_green().white(),
                    device_id,
                    hog_pid
                );
                Ok(true)
            } else {
                warn!(
                    "{} Device {} already hogged by PID {}",
                    "HOG_BUSY".on_yellow().white(),
                    device_id,
                    hog_pid
                );
                Ok(false)
            }
        }
    }

    /// Release exclusive control of an audio device
    pub fn release_hog(device_id: AudioObjectID) -> Result<()> {
        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_DEVICE_PROPERTY_HOG_MODE,
                mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
            };

            // Release hog by setting to -1
            let mut hog_pid: i32 = -1;
            let size = std::mem::size_of::<i32>() as u32;

            let status: OSStatus = AudioObjectSetPropertyData(
                device_id,
                &address,
                0,
                std::ptr::null(),
                size,
                &mut hog_pid as *mut i32 as *mut c_void,
            );

            if status != 0 {
                error!(
                    "{} Failed to release hog on device {}: OSStatus {}",
                    "HOG_ERROR".on_red().white(),
                    device_id,
                    status
                );
                return Err(anyhow::anyhow!(
                    "Failed to release hog on device {}: OSStatus {}",
                    device_id,
                    status
                ));
            }

            info!(
                "{} Released hog on device {}",
                "HOG_RELEASED".on_blue().white(),
                device_id
            );

            Ok(())
        }
    }

    /// Check who currently has the device hogged (returns PID or -1 if not hogged)
    pub fn get_hog_owner(device_id: AudioObjectID) -> Result<i32> {
        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_DEVICE_PROPERTY_HOG_MODE,
                mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
            };

            let mut hog_pid: i32 = -1;
            let mut size = std::mem::size_of::<i32>() as u32;

            let status: OSStatus = AudioObjectGetPropertyData(
                device_id,
                &address,
                0,
                std::ptr::null(),
                &mut size,
                &mut hog_pid as *mut i32 as *mut c_void,
            );

            if status != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to get hog status for device {}: OSStatus {}",
                    device_id,
                    status
                ));
            }

            Ok(hog_pid)
        }
    }

    /// Check if we (this process) currently have the device hogged
    pub fn is_hogged_by_us(device_id: AudioObjectID) -> bool {
        match Self::get_hog_owner(device_id) {
            Ok(hog_pid) => hog_pid == std::process::id() as i32,
            Err(_) => false,
        }
    }
}
