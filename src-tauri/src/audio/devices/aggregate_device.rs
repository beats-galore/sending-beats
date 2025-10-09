use crate::audio::tap::core_audio_bindings::{
    AudioHardwareCreateAggregateDeviceFromDict, AudioHardwareDestroyAggregateDevice, AudioObjectID,
    OSStatus,
};
use anyhow::{Context, Result};
use colored::Colorize;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use std::ptr;
use tracing::{error, info};

const KAUDIO_AGGREGATE_DEVICE_UID_KEY: &str = "uid";
const KAUDIO_AGGREGATE_DEVICE_NAME_KEY: &str = "name";
const KAUDIO_AGGREGATE_DEVICE_SUB_DEVICE_LIST_KEY: &str = "subdevices";
const KAUDIO_AGGREGATE_DEVICE_IS_PRIVATE_KEY: &str = "private";
const KAUDIO_AGGREGATE_DEVICE_IS_STACKED_KEY: &str = "stacked";

/// Manager for creating and destroying silent aggregate audio devices
pub struct AggregateDeviceManager;

impl AggregateDeviceManager {
    /// Create a silent aggregate device with no sub-devices
    /// This device will accept audio but not output it anywhere
    pub fn create_silent_aggregate_device(
        name: &str,
        uid: &str,
    ) -> Result<(AudioObjectID, String)> {
        info!(
            "{} Creating silent aggregate device: name='{}', uid='{}'",
            "AGGREGATE_CREATE".bright_cyan(),
            name,
            uid
        );

        unsafe {
            let cf_name = CFString::new(name);
            let cf_uid = CFString::new(uid);
            let cf_private = CFNumber::from(1i32);
            let cf_stacked = CFNumber::from(1i32);

            let mut dict = CFMutableDictionary::with_capacity(4);
            dict.set(
                CFString::new(KAUDIO_AGGREGATE_DEVICE_NAME_KEY).as_CFType(),
                cf_name.as_CFType(),
            );
            dict.set(
                CFString::new(KAUDIO_AGGREGATE_DEVICE_UID_KEY).as_CFType(),
                cf_uid.as_CFType(),
            );
            dict.set(
                CFString::new(KAUDIO_AGGREGATE_DEVICE_IS_PRIVATE_KEY).as_CFType(),
                cf_private.as_CFType(),
            );
            dict.set(
                CFString::new(KAUDIO_AGGREGATE_DEVICE_IS_STACKED_KEY).as_CFType(),
                cf_stacked.as_CFType(),
            );

            let mut device_id: AudioObjectID = 0;
            let status: OSStatus = AudioHardwareCreateAggregateDeviceFromDict(
                dict.as_concrete_TypeRef(),
                &mut device_id as *mut AudioObjectID,
            );

            if status != 0 {
                error!(
                    "{} Failed to create aggregate device: OSStatus {}",
                    "AGGREGATE_ERROR".bright_red(),
                    status
                );
                return Err(anyhow::anyhow!(
                    "Failed to create aggregate device: OSStatus {}",
                    status
                ));
            }

            info!(
                "{} Successfully created silent aggregate device with ID: {}",
                "AGGREGATE_CREATED".bright_green(),
                device_id
            );

            Ok((device_id, uid.to_string()))
        }
    }

    /// Destroy an aggregate device by its AudioObjectID
    pub fn destroy_aggregate_device(device_id: AudioObjectID) -> Result<()> {
        info!(
            "{} Destroying aggregate device with ID: {}",
            "AGGREGATE_DESTROY".bright_yellow(),
            device_id
        );

        unsafe {
            let status: OSStatus = AudioHardwareDestroyAggregateDevice(device_id);

            if status != 0 {
                error!(
                    "{} Failed to destroy aggregate device {}: OSStatus {}",
                    "AGGREGATE_ERROR".bright_red(),
                    device_id,
                    status
                );
                return Err(anyhow::anyhow!(
                    "Failed to destroy aggregate device: OSStatus {}",
                    status
                ));
            }

            info!(
                "{} Successfully destroyed aggregate device",
                "AGGREGATE_DESTROYED".bright_green()
            );

            Ok(())
        }
    }

    /// Check if an aggregate device exists by attempting to get its properties
    pub fn device_exists(device_id: AudioObjectID) -> bool {
        use crate::audio::tap::core_audio_bindings::{
            AudioObjectGetPropertyData, AudioObjectPropertyAddress,
        };

        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: 1735354734, // 'glob' for kAudioObjectPropertyOwnedObjects
                mScope: 0,             // kAudioObjectPropertyScopeGlobal
                mElement: 0,           // kAudioObjectPropertyElementMain
            };

            let mut data_size: u32 = 0;
            let status = AudioObjectGetPropertyData(
                device_id,
                &address,
                0,
                ptr::null(),
                &mut data_size,
                ptr::null_mut(),
            );

            status == 0
        }
    }

    /// Verify aggregate device exists by UID (searches all devices)
    pub fn verify_device_by_uid(uid: &str) -> Option<AudioObjectID> {
        use crate::audio::tap::core_audio_bindings::{
            kAudioObjectSystemObject, AudioObjectGetPropertyData, AudioObjectPropertyAddress,
        };
        use std::ffi::c_void;

        info!(
            "{} Verifying aggregate device existence by UID: {}",
            "AGGREGATE_VERIFY".bright_magenta(),
            uid
        );

        unsafe {
            let translate_address = AudioObjectPropertyAddress {
                mSelector: 1969841252, // 'uidd' for kAudioHardwarePropertyTranslateUIDToDevice
                mScope: 0,             // kAudioObjectPropertyScopeGlobal
                mElement: 0,           // kAudioObjectPropertyElementMain
            };

            let cf_uid = CFString::new(uid);
            let mut device_id: AudioObjectID = 0;
            let mut data_size = std::mem::size_of::<AudioObjectID>() as u32;

            let status = AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &translate_address,
                std::mem::size_of::<CFString>() as u32,
                &cf_uid as *const _ as *const c_void,
                &mut data_size,
                &mut device_id as *mut AudioObjectID as *mut c_void,
            );

            if status == 0 && device_id != 0 {
                info!(
                    "{} Found aggregate device with UID '{}': ID {}",
                    "AGGREGATE_FOUND".bright_green(),
                    uid,
                    device_id
                );
                Some(device_id)
            } else {
                info!(
                    "{} Aggregate device with UID '{}' not found",
                    "AGGREGATE_NOT_FOUND".bright_yellow(),
                    uid
                );
                None
            }
        }
    }
}
