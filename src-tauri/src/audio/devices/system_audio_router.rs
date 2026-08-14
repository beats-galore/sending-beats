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

// Taken from the generated Core Audio bindings rather than written out as
// four-character codes. A selector transcribed by hand compiles regardless of
// whether it names a real property, and only fails at runtime as a bad object.
use coreaudio_sys::{
    kAudioDevicePropertyDeviceUID as KAUDIO_DEVICE_PROPERTY_DEVICE_UID,
    kAudioHardwarePropertyDefaultOutputDevice as KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
    kAudioHardwarePropertyDefaultSystemOutputDevice as KAUDIO_HARDWARE_PROPERTY_DEFAULT_SYSTEM_OUTPUT_DEVICE,
    kAudioHardwarePropertyTranslateUIDToDevice as KAUDIO_HARDWARE_PROPERTY_TRANSLATE_UID_TO_DEVICE,
    kAudioObjectPropertyElementMain as KAUDIO_OBJECT_PROPERTY_ELEMENT_MASTER,
    kAudioObjectPropertyScopeGlobal as KAUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
};

const DEFAULT_OUTPUT_LABEL: &str = "default output";
const SYSTEM_OUTPUT_LABEL: &str = "system output";

/// Result of routing system audio through the virtual driver
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiversionOutcome {
    /// System output is now the virtual driver
    Diverted,
    /// The driver was installed or reloaded, which restarts coreaudiod and
    /// invalidates this process's Core Audio client. The new device stays
    /// invisible to us until the app is relaunched, so diversion cannot finish
    /// in this run.
    RestartRequired,
}

/// System audio routing manager
/// Handles diverting system audio to the virtual driver and restoring the original default
pub struct SystemAudioRouter {
    db: DatabaseConnection,
}

impl SystemAudioRouter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Get the current system default output device UID
    pub fn get_current_default_output_uid(&self) -> Result<String> {
        self.get_default_uid(
            KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
            DEFAULT_OUTPUT_LABEL,
        )
    }

    fn get_default_uid(&self, selector: u32, label: &str) -> Result<String> {
        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: selector,
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
                    "{} Failed to get current {} device: OSStatus {}",
                    "SYS_AUDIO_ERROR".bright_red(),
                    label,
                    status
                );
                return Err(anyhow::anyhow!(
                    "Failed to get {} device: OSStatus {}",
                    label,
                    status
                ));
            }

            let uid = self.get_device_uid_from_id(device_id)?;
            info!(
                "{} Current {} device: UID='{}' (ID={})",
                "SYS_AUDIO_QUERY".bright_blue(),
                label,
                uid,
                device_id
            );

            Ok(uid)
        }
    }

    /// Wait for a default-output selector to report `expected_uid`.
    ///
    /// coreaudiod applies the change asynchronously, so the first read back can
    /// still show the old device.
    fn await_default(&self, selector: u32, label: &str, expected_uid: &str) -> Result<String> {
        let mut actual = self.get_default_uid(selector, label)?;

        for _ in 0..10 {
            if actual == expected_uid {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            actual = self.get_default_uid(selector, label)?;
        }

        Ok(actual)
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

    /// Check that both default-output selectors now point at the virtual device.
    ///
    /// Both matter, for different reasons. The default output is where audio
    /// plays, and diverting it is what stops system audio doubling up with the
    /// mix. The system output is what the volume and mute keys act on, and
    /// leaving it on a physical device means a mute sets that device's own mute
    /// property — which silences the mixer's stream too, since a device mute
    /// applies underneath every app playing through it.
    fn verify_diversion(&self, virtual_device_uid: &str) -> Result<()> {
        let checks = [
            (
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
                DEFAULT_OUTPUT_LABEL,
            ),
            (
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_SYSTEM_OUTPUT_DEVICE,
                SYSTEM_OUTPUT_LABEL,
            ),
        ];

        for (selector, label) in checks {
            let actual = self.await_default(selector, label, virtual_device_uid)?;
            if actual != virtual_device_uid {
                error!(
                    "{} Failed to divert {}. Expected '{}' but system reports '{}'",
                    "SYS_AUDIO_ERROR".bright_red(),
                    label,
                    virtual_device_uid,
                    actual
                );
                return Err(anyhow::anyhow!(
                    "Failed to set virtual device as {}. Expected '{}' but got '{}'",
                    label,
                    virtual_device_uid,
                    actual
                ));
            }
        }

        Ok(())
    }

    /// Ensure system audio is routed to the virtual driver so the physical output is free
    pub async fn divert_system_audio_to_virtual_device(&mut self) -> Result<DiversionOutcome> {
        // Installing restarts coreaudiod, so stop here rather than chasing a
        // device this process can no longer see. Continuing would only raise a
        // second authorization prompt for a lookup that cannot succeed yet.
        if !VirtualDriverManager::is_installed() {
            info!(
                "{} Virtual driver not installed, installing now...",
                "SYS_AUDIO_INSTALL".bright_cyan()
            );
            VirtualDriverManager::install().await?;

            return Ok(DiversionOutcome::RestartRequired);
        }

        VirtualDriverManager::verify_installation()?;

        // The bundle can be on disk while coreaudiod has never published the
        // device, which leaves the driver permanently unloaded because the
        // install path short-circuits on the files already existing
        let virtual_device_uid = match VirtualDriverManager::get_device_uid().await {
            Ok(uid) => uid,
            Err(e) => {
                warn!(
                    "{} Virtual device not published ({}), restarting coreaudiod",
                    "SYS_AUDIO_RELOAD".bright_yellow(),
                    e
                );

                VirtualDriverManager::reload_coreaudiod().await?;

                return Ok(DiversionOutcome::RestartRequired);
            }
        };
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

        self.verify_diversion(&virtual_device_uid)?;

        SystemAudioStateService::set_diversion_state(&self.db, true, previous_default_uid.clone())
            .await?;

        info!(
            "{} System audio now routed to virtual device '{}' (silent output)",
            "SYS_AUDIO_DIVERTED".bright_green(),
            virtual_device_uid
        );

        Ok(DiversionOutcome::Diverted)
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
}
