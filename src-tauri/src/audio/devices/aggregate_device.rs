use crate::audio::tap::core_audio_bindings::{
    AudioHardwareCreateAggregateDeviceFromDict, AudioHardwareDestroyAggregateDevice, AudioObjectID,
    OSStatus,
};
use anyhow::{Context, Result};
use colored::Colorize;
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFMutableDictionary};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use std::ptr;
use tracing::{error, info};

const KAUDIO_AGGREGATE_DEVICE_UID_KEY: &str = "uid";
const KAUDIO_AGGREGATE_DEVICE_NAME_KEY: &str = "name";
const KAUDIO_AGGREGATE_DEVICE_SUB_DEVICE_LIST_KEY: &str = "subdevices";
const KAUDIO_AGGREGATE_DEVICE_IS_PRIVATE_KEY: &str = "private";
const KAUDIO_AGGREGATE_DEVICE_IS_STACKED_KEY: &str = "stacked";
const KAUDIO_AGGREGATE_DEVICE_MAIN_SUB_DEVICE_KEY: &str = "master";
const KAUDIO_SUB_DEVICE_UID_KEY: &str = "uid";

const KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = 1735159650; // 'glob' (0x676C6F62)
const KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER: u32 = 0; // 0x00000000

/// Manager for creating and destroying silent aggregate audio devices
pub struct AggregateDeviceManager;

impl AggregateDeviceManager {
    /// Create a silent aggregate device with no sub-devices
    /// This device will accept audio but not output it anywhere
    pub fn create_silent_aggregate_device(
        name: &str,
        uid: &str,
    ) -> Result<(AudioObjectID, String)> {
        Self::create_custom_aggregate_device(name, uid, &[], false)
    }

    /// Create a aggregate device that proxies specific hardware sub-devices
    pub fn create_private_aggregate_device(
        name: &str,
        uid: &str,
        sub_device_uids: &[&str],
    ) -> Result<(AudioObjectID, String)> {
        Self::create_custom_aggregate_device(name, uid, sub_device_uids, false)
    }

    fn create_custom_aggregate_device(
        name: &str,
        uid: &str,
        sub_device_uids: &[&str],
        is_private: bool,
    ) -> Result<(AudioObjectID, String)> {
        if sub_device_uids.is_empty() {
            info!(
                "{} Creating silent aggregate device: name='{}', uid='{}'",
                "AGGREGATE_CREATE".bright_cyan(),
                name,
                uid
            );
        } else {
            info!(
                "{} Creating private aggregate device: name='{}', uid='{}', sub_devices={:?}",
                "AGGREGATE_CREATE".bright_cyan(),
                name,
                uid,
                sub_device_uids
            );
        }

        unsafe {
            let cf_name = CFString::new(name);
            let cf_uid = CFString::new(uid);
            let cf_private = CFNumber::from(if is_private { 1i32 } else { 0i32 });
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

            let subdevice_cf_strings: Vec<CFString> = sub_device_uids
                .iter()
                .map(|uid| CFString::new(uid))
                .collect();

            if !subdevice_cf_strings.is_empty() {
                let subdevice_dictionaries: Vec<CFDictionary<CFType, CFType>> =
                    subdevice_cf_strings
                        .iter()
                        .map(|cf_uid_string| {
                            let mut sub_dict = CFMutableDictionary::with_capacity(1);
                            sub_dict.set(
                                CFString::new(KAUDIO_SUB_DEVICE_UID_KEY).as_CFType(),
                                cf_uid_string.as_CFType(),
                            );
                            sub_dict.to_immutable()
                        })
                        .collect();

                let subdevices_array =
                    CFArray::<CFDictionary<CFType, CFType>>::from_CFTypes(&subdevice_dictionaries);
                dict.set(
                    CFString::new(KAUDIO_AGGREGATE_DEVICE_SUB_DEVICE_LIST_KEY).as_CFType(),
                    subdevices_array.as_CFType(),
                );

                // Use the first sub-device as the master clock by default
                let master_uid = subdevice_cf_strings[0].clone();
                dict.set(
                    CFString::new(KAUDIO_AGGREGATE_DEVICE_MAIN_SUB_DEVICE_KEY).as_CFType(),
                    master_uid.as_CFType(),
                );
            }

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
                "{} Successfully created aggregate device with ID: {}",
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
                mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
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
                mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
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
