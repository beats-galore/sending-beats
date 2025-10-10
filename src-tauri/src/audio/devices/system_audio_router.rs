use crate::audio::devices::aggregate_device::AggregateDeviceManager;
use crate::audio::devices::virtual_driver::VirtualDriverManager;
use crate::audio::tap::core_audio_bindings::{
    kAudioObjectSystemObject, AudioObjectGetPropertyData, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectSetPropertyData, OSStatus,
};
use crate::db::SystemAudioStateService;
use anyhow::{Context, Result};
use colored::Colorize;
use core_foundation::string::CFString;
use sea_orm::DatabaseConnection;
use std::ffi::c_void;
use std::ptr;
use tracing::{error, info, warn};
use uuid::Uuid;

const KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE: u32 = 1682929012; // 'dOut' (0x646F7574)
const KAUDIO_HARDWARE_PROPERTY_DEFAULT_SYSTEM_OUTPUT_DEVICE: u32 = 1936747636; // 'sOut' (0x734F7574)
const KAUDIO_HARDWARE_PROPERTY_TRANSLATE_UID_TO_DEVICE: u32 = 1969841252; // 'uidd' (0x75696464)
const KAUDIO_DEVICE_PROPERTY_DEVICE_UID: u32 = 1969841184; // 'uid ' (0x75696420)
const KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = 1735159650; // 'glob' (0x676C6F62)
const KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER: u32 = 0; // 0x00000000

/// System audio routing manager
/// Handles diverting system audio to a dummy aggregate device and restoring the original default
pub struct SystemAudioRouter {
    db: DatabaseConnection,
}

impl SystemAudioRouter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Get the current system default output device UID
    pub fn get_current_default_output_uid(&self) -> Result<String> {
        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
                mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
            };

            let mut device_id: AudioObjectID = 0;
            let mut data_size = std::mem::size_of::<AudioObjectID>() as u32;

            let status = AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &address,
                0,
                ptr::null(),
                &mut data_size,
                &mut device_id as *mut AudioObjectID as *mut c_void,
            );

            if status != 0 {
                error!(
                    "{} Failed to get current default output device: OSStatus {}",
                    "SYS_AUDIO_ERROR".bright_red(),
                    status
                );
                return Err(anyhow::anyhow!(
                    "Failed to get default output device: OSStatus {}",
                    status
                ));
            }

            let uid = self.get_device_uid_from_id(device_id)?;
            info!(
                "{} Current default output device: UID='{}' (ID={})",
                "SYS_AUDIO_QUERY".bright_blue(),
                uid,
                device_id
            );

            Ok(uid)
        }
    }

    /// Get device UID from its AudioObjectID
    fn get_device_uid_from_id(&self, device_id: AudioObjectID) -> Result<String> {
        use core_foundation::base::TCFType;

        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_DEVICE_PROPERTY_DEVICE_UID,
                mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
            };

            let mut cf_uid: *mut core_foundation::string::__CFString = ptr::null_mut();
            let mut data_size =
                std::mem::size_of::<*mut core_foundation::string::__CFString>() as u32;

            let status = AudioObjectGetPropertyData(
                device_id,
                &address,
                0,
                ptr::null(),
                &mut data_size,
                &mut cf_uid as *mut _ as *mut c_void,
            );

            if status != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to get device UID: OSStatus {}",
                    status
                ));
            }

            let cf_string = CFString::wrap_under_create_rule(cf_uid);
            Ok(cf_string.to_string())
        }
    }

    /// Set the system default output device by UID
    fn set_default_output_device(&self, device_uid: &str) -> Result<()> {
        unsafe {
            let device_id = self.translate_uid_to_device_id(device_uid)?;

            // Try setting both Default and System output properties
            // Some macOS versions respect one vs the other
            let properties = [
                ("Default", KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE),
                (
                    "System",
                    KAUDIO_HARDWARE_PROPERTY_DEFAULT_SYSTEM_OUTPUT_DEVICE,
                ),
            ];

            let mut any_succeeded = false;
            for (name, selector) in &properties {
                let address = AudioObjectPropertyAddress {
                    mSelector: *selector,
                    mScope: KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                    mElement: KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
                };

                let status = AudioObjectSetPropertyData(
                    kAudioObjectSystemObject,
                    &address,
                    0,
                    ptr::null(),
                    std::mem::size_of::<AudioObjectID>() as u32,
                    &device_id as *const AudioObjectID as *const c_void,
                );

                if status == 0 {
                    info!(
                        "{} Set {} output device to: UID='{}'",
                        "SYS_AUDIO_SET".bright_green(),
                        name,
                        device_uid
                    );
                    any_succeeded = true;
                } else {
                    warn!(
                        "{} Failed to set {} output device to UID '{}': OSStatus {}",
                        "SYS_AUDIO_WARN".bright_yellow(),
                        name,
                        device_uid,
                        status
                    );
                }
            }

            if !any_succeeded {
                return Err(anyhow::anyhow!("Failed to set any output device property"));
            }

            Ok(())
        }
    }

    /// Translate device UID to AudioObjectID
    fn translate_uid_to_device_id(&self, uid: &str) -> Result<AudioObjectID> {
        use core_foundation::base::TCFType;

        unsafe {
            let translate_address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_HARDWARE_PROPERTY_TRANSLATE_UID_TO_DEVICE,
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

            if status != 0 || device_id == 0 {
                return Err(anyhow::anyhow!(
                    "Failed to translate UID '{}' to device ID: OSStatus {}",
                    uid,
                    status
                ));
            }

            Ok(device_id)
        }
    }

    /// Ensure system audio is routed to the virtual driver so the physical output is free
    pub async fn divert_system_audio_to_virtual_device(&mut self) -> Result<()> {
        // Ensure virtual driver is installed
        if !VirtualDriverManager::is_installed() {
            info!(
                "{} Virtual driver not installed, installing now...",
                "SYS_AUDIO_INSTALL".bright_cyan()
            );
            VirtualDriverManager::install().await?;
        }

        VirtualDriverManager::verify_installation()?;

        let virtual_device_uid = VirtualDriverManager::get_device_uid()
            .await
            .context("Failed to get virtual device UID")?;
        let state = SystemAudioStateService::get_or_create(&self.db).await?;

        // Save current default if not already diverted
        let mut previous_default_uid = state.previous_default_device_uid.clone();
        if !state.is_diverted || previous_default_uid.is_none() {
            let current_default = self.get_current_default_output_uid()?;
            info!(
                "{} Caching previous default output device '{}'",
                "SYS_AUDIO_SAVE".bright_blue(),
                current_default
            );
            previous_default_uid = Some(current_default);
        }

        // Set virtual device as system default
        info!(
            "{} Setting virtual device '{}' as system default output",
            "SYS_AUDIO_DIVERT".bright_cyan(),
            virtual_device_uid
        );

        self.set_default_output_device(&virtual_device_uid)?;

        // Verify the change took effect
        let mut actual_default = self.get_current_default_output_uid()?;
        if actual_default != virtual_device_uid {
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                actual_default = self.get_current_default_output_uid()?;
                if actual_default == virtual_device_uid {
                    break;
                }
            }
        }

        if actual_default != virtual_device_uid {
            error!(
                "{} Failed to divert system audio. Expected '{}' but system reports '{}'",
                "SYS_AUDIO_ERROR".bright_red(),
                virtual_device_uid,
                actual_default
            );
            return Err(anyhow::anyhow!(
                "Failed to set virtual device as system default. Expected '{}' but got '{}'",
                virtual_device_uid,
                actual_default
            ));
        }

        SystemAudioStateService::set_diversion_state(&self.db, true, previous_default_uid.clone())
            .await?;

        info!(
            "{} System audio now routed to virtual device '{}' (silent output)",
            "SYS_AUDIO_DIVERTED".bright_green(),
            virtual_device_uid
        );

        Ok(())
    }

    /// Restore the original default output device
    pub async fn restore_original_default(&mut self) -> Result<()> {
        let state = SystemAudioStateService::get_or_create(&self.db).await?;

        if !state.is_diverted {
            warn!(
                "{} System audio not currently diverted, skipping restore",
                "SYS_AUDIO_WARN".bright_yellow()
            );
            return Ok(());
        }

        if let Some(previous_uid) = &state.previous_default_device_uid {
            info!(
                "{} Restoring previous default device: '{}'",
                "SYS_AUDIO_RESTORE".bright_magenta(),
                previous_uid
            );

            if let Err(e) = self.set_default_output_device(previous_uid) {
                warn!(
                    "{} Failed to restore device '{}': {}. Falling back to system default",
                    "SYS_AUDIO_WARN".bright_yellow(),
                    previous_uid,
                    e
                );
            } else {
                info!(
                    "{} Successfully restored system audio to '{}'",
                    "SYS_AUDIO_RESTORED".bright_green(),
                    previous_uid
                );
            }
        } else {
            warn!(
                "{} No previous default device saved, skipping restore",
                "SYS_AUDIO_WARN".bright_yellow()
            );
        }

        SystemAudioStateService::reset_diversion(&self.db).await?;

        if let Some(aggregate_uid) = state.dummy_aggregate_device_uid {
            if let Some(aggregate_id) = AggregateDeviceManager::verify_device_by_uid(&aggregate_uid)
            {
                info!(
                    "{} Destroying silent aggregate '{}'",
                    "SYS_AUDIO_CLEANUP".bright_cyan(),
                    aggregate_uid
                );
                if let Err(e) = AggregateDeviceManager::destroy_aggregate_device(aggregate_id) {
                    warn!(
                        "{} Failed to destroy aggregate device '{}' during restore: {}",
                        "SYS_AUDIO_WARN".bright_yellow(),
                        aggregate_uid,
                        e
                    );
                }
            }
            SystemAudioStateService::set_dummy_device_uid(&self.db, None).await?;
        }

        Ok(())
    }

    async fn create_and_save_silent_device(&self) -> Result<String> {
        let uid = format!("sendin-beats-silent-{}", Uuid::new_v4());
        let name = "Sendin Beats Silent Output";

        let (_device_id, device_uid) =
            AggregateDeviceManager::create_silent_aggregate_device(name, &uid)?;

        std::thread::sleep(std::time::Duration::from_millis(100));

        SystemAudioStateService::set_dummy_device_uid(&self.db, Some(device_uid.clone())).await?;

        info!(
            "{} Created and saved silent aggregate device: '{}'",
            "SYS_AUDIO_CREATED".bright_green(),
            device_uid
        );

        Ok(device_uid)
    }

    pub async fn ensure_dummy_device_exists(&mut self) -> Result<String> {
        let state = SystemAudioStateService::get_or_create(&self.db).await?;

        if let Some(uid) = &state.dummy_aggregate_device_uid {
            if AggregateDeviceManager::verify_device_by_uid(uid).is_some() {
                return Ok(uid.clone());
            }
        }

        self.create_and_save_silent_device().await
    }
}
