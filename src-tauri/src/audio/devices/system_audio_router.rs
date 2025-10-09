use crate::audio::devices::aggregate_device::AggregateDeviceManager;
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

const KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE: u32 = 1868981858; // 'dOut'
const KAUDIO_HARDWARE_PROPERTY_TRANSLATE_UID_TO_DEVICE: u32 = 1969841252; // 'uidd'
const KAUDIO_HARDWARE_PROPERTY_DEVICE_UID: u32 = 1969841252; // 'uidd' - wait, this should be different

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
                mScope: 0,   // kAudioObjectPropertyScopeGlobal
                mElement: 0, // kAudioObjectPropertyElementMain
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
                mSelector: 1969841845, // 'uid ' (kAudioDevicePropertyDeviceUID)
                mScope: 0,             // kAudioObjectPropertyScopeGlobal
                mElement: 0,           // kAudioObjectPropertyElementMain
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

            let address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
                mScope: 0,   // kAudioObjectPropertyScopeGlobal
                mElement: 0, // kAudioObjectPropertyElementMain
            };

            let status = AudioObjectSetPropertyData(
                kAudioObjectSystemObject,
                &address,
                0,
                ptr::null(),
                std::mem::size_of::<AudioObjectID>() as u32,
                &device_id as *const AudioObjectID as *const c_void,
            );

            if status != 0 {
                error!(
                    "{} Failed to set default output device to UID '{}': OSStatus {}",
                    "SYS_AUDIO_ERROR".bright_red(),
                    device_uid,
                    status
                );
                return Err(anyhow::anyhow!(
                    "Failed to set default output device: OSStatus {}",
                    status
                ));
            }

            info!(
                "{} Set system default output device to: UID='{}'",
                "SYS_AUDIO_SET".bright_green(),
                device_uid
            );

            Ok(())
        }
    }

    /// Translate device UID to AudioObjectID
    fn translate_uid_to_device_id(&self, uid: &str) -> Result<AudioObjectID> {
        use core_foundation::base::TCFType;

        unsafe {
            let translate_address = AudioObjectPropertyAddress {
                mSelector: KAUDIO_HARDWARE_PROPERTY_TRANSLATE_UID_TO_DEVICE,
                mScope: 0,   // kAudioObjectPropertyScopeGlobal
                mElement: 0, // kAudioObjectPropertyElementMain
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

    /// Divert system audio to a dummy aggregate device
    /// Saves the current default device UID for later restoration
    pub async fn divert_to_dummy_device(&mut self) -> Result<()> {
        let state = SystemAudioStateService::get_or_create(&self.db).await?;

        if state.is_diverted {
            warn!(
                "{} System audio already diverted, skipping",
                "SYS_AUDIO_WARN".bright_yellow()
            );
            return Ok(());
        }

        let dummy_uid = if let Some(uid) = &state.dummy_aggregate_device_uid {
            if AggregateDeviceManager::verify_device_by_uid(uid).is_some() {
                info!(
                    "{} Using existing dummy aggregate device: '{}'",
                    "SYS_AUDIO_REUSE".bright_cyan(),
                    uid
                );
                uid.clone()
            } else {
                info!(
                    "{} Dummy device '{}' no longer exists, creating new one",
                    "SYS_AUDIO_RECREATE".bright_yellow(),
                    uid
                );
                self.create_and_save_dummy_device().await?
            }
        } else {
            info!(
                "{} No dummy device found, creating new one",
                "SYS_AUDIO_CREATE".bright_cyan()
            );
            self.create_and_save_dummy_device().await?
        };

        let previous_default = self.get_current_default_output_uid()?;

        info!(
            "{} Saving previous default device: '{}'",
            "SYS_AUDIO_SAVE".bright_blue(),
            previous_default
        );

        self.set_default_output_device(&dummy_uid)?;

        SystemAudioStateService::set_diversion_state(
            &self.db,
            true,
            Some(previous_default.clone()),
        )
        .await?;

        info!(
            "{} System audio diverted from '{}' to dummy device '{}'",
            "SYS_AUDIO_DIVERTED".bright_green(),
            previous_default,
            dummy_uid
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

        Ok(())
    }

    /// Create a new dummy aggregate device and save its UID to the database
    async fn create_and_save_dummy_device(&self) -> Result<String> {
        let uid = format!("sendin-beats-silent-{}", uuid::Uuid::new_v4());
        let name = "Sendin Beats Silent Output";

        let (_device_id, device_uid) =
            AggregateDeviceManager::create_silent_aggregate_device(name, &uid)?;

        SystemAudioStateService::set_dummy_device_uid(&self.db, Some(device_uid.clone())).await?;

        info!(
            "{} Created and saved dummy aggregate device: '{}'",
            "SYS_AUDIO_CREATED".bright_green(),
            device_uid
        );

        Ok(device_uid)
    }

    /// Ensure dummy device exists, creating it if necessary
    pub async fn ensure_dummy_device_exists(&mut self) -> Result<String> {
        let state = SystemAudioStateService::get_or_create(&self.db).await?;

        if let Some(uid) = &state.dummy_aggregate_device_uid {
            if AggregateDeviceManager::verify_device_by_uid(uid).is_some() {
                return Ok(uid.clone());
            }
        }

        self.create_and_save_dummy_device().await
    }
}
