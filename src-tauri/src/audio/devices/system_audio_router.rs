use crate::audio::devices::virtual_driver::VirtualDriverManager;
use crate::audio::tap::core_audio_bindings::{
    kAudioObjectSystemObject, AudioObjectGetPropertyData, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectSetPropertyData, OSStatus,
};
use crate::db::SystemAudioStateService;
use crate::entities::system_audio_state;
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

/// The devices diversion displaced, each to be put back on its own selector
#[derive(Debug, Clone, Default)]
struct PreviousDefaults {
    default_output: Option<String>,
    system_output: Option<String>,
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
    /// Point one of the default-output selectors at a device
    fn set_output_device(&self, selector: u32, label: &str, device_uid: &str) -> Result<()> {
        unsafe {
            let device_id = self.translate_uid_to_device_id(device_uid)?;

            let address = AudioObjectPropertyAddress {
                mSelector: selector,
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

            if status != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to set {} device to UID '{}': OSStatus {}",
                    label,
                    device_uid,
                    status
                ));
            }

            info!(
                "{} Set {} device to: UID='{}'",
                "SYS_AUDIO_SET".bright_green(),
                label,
                device_uid
            );

            Ok(())
        }
    }

    /// Point both default-output selectors at the same device
    ///
    /// A failure here is not decisive on its own — coreaudiod can reject a set
    /// that succeeds a moment later, so the caller verifies rather than trusting
    /// the status.
    fn set_both_output_devices(&self, device_uid: &str) {
        let targets = [
            (
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
                DEFAULT_OUTPUT_LABEL,
            ),
            (
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_SYSTEM_OUTPUT_DEVICE,
                SYSTEM_OUTPUT_LABEL,
            ),
        ];

        for (selector, label) in targets {
            if let Err(e) = self.set_output_device(selector, label, device_uid) {
                warn!("{} {}", "SYS_AUDIO_WARN".bright_yellow(), e);
            }
        }
    }

    /// Take the devices being diverted from off mute
    ///
    /// Not restored on undiversion. Handing a device back muted is exactly how
    /// this went wrong to begin with: the machine is left silent with nothing
    /// anywhere to explain it, where unmuted is the state a person can hear and
    /// undo for themselves.
    fn unmute_diverted_devices(&self, previous: &PreviousDefaults) {
        let uids = [
            previous.default_output.as_deref(),
            previous.system_output.as_deref(),
        ];

        for uid in uids.into_iter().flatten() {
            let Ok(device_id) = self.translate_uid_to_device_id(uid) else {
                continue;
            };

            if crate::audio::devices::output_health::clear_mute(device_id) {
                info!(
                    "{} Took '{}' off mute before diverting away from it",
                    "SYS_AUDIO_DIVERT".bright_cyan(),
                    uid
                );
            }
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

    /// The devices to put back when diversion ends.
    ///
    /// An already-diverted record wins over what the system currently reports:
    /// reading again while diverted would cache the virtual device and strand
    /// the real one.
    fn devices_to_restore(&self, state: &system_audio_state::Model) -> Result<PreviousDefaults> {
        let mut previous = PreviousDefaults {
            default_output: state.previous_default_device_uid.clone(),
            system_output: state.previous_system_output_device_uid.clone(),
        };

        if !state.is_diverted || previous.default_output.is_none() {
            previous.default_output = Some(self.get_default_uid(
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
                DEFAULT_OUTPUT_LABEL,
            )?);
        }

        if !state.is_diverted || previous.system_output.is_none() {
            previous.system_output = Some(self.get_default_uid(
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_SYSTEM_OUTPUT_DEVICE,
                SYSTEM_OUTPUT_LABEL,
            )?);
        }

        info!(
            "{} Caching previous devices — {}: '{:?}', {}: '{:?}'",
            "SYS_AUDIO_SAVE".bright_blue(),
            DEFAULT_OUTPUT_LABEL,
            previous.default_output,
            SYSTEM_OUTPUT_LABEL,
            previous.system_output
        );

        Ok(previous)
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
        let previous = self.devices_to_restore(&state)?;

        // Set virtual device as system default
        info!(
            "{} Setting virtual device '{}' as system default output",
            "SYS_AUDIO_DIVERT".bright_cyan(),
            virtual_device_uid
        );

        // Before the selectors move: whatever the volume keys did to these
        // devices is about to become unreachable, because the keys will be
        // acting on the virtual driver instead. A mute left behind here is one
        // nothing can clear by hand, and it silences the mixer's own stream if
        // that device is later patched in as a destination.
        self.unmute_diverted_devices(&previous);

        self.set_both_output_devices(&virtual_device_uid);

        self.verify_diversion(&virtual_device_uid)?;

        SystemAudioStateService::set_diversion_state(
            &self.db,
            true,
            previous.default_output.clone(),
            previous.system_output.clone(),
        )
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

        // Each selector goes back to its own device. They are free to differ —
        // the sound-effects device is chosen separately in Sound settings — and
        // restoring one to the other's value would quietly rewrite that choice.
        let restores = [
            (
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
                DEFAULT_OUTPUT_LABEL,
                &state.previous_default_device_uid,
            ),
            (
                KAUDIO_HARDWARE_PROPERTY_DEFAULT_SYSTEM_OUTPUT_DEVICE,
                SYSTEM_OUTPUT_LABEL,
                &state.previous_system_output_device_uid,
            ),
        ];

        for (selector, label, previous) in restores {
            let Some(previous_uid) = previous else {
                warn!(
                    "{} No previous {} device saved, leaving it as it is",
                    "SYS_AUDIO_WARN".bright_yellow(),
                    label
                );
                continue;
            };

            info!(
                "{} Restoring previous {} device: '{}'",
                "SYS_AUDIO_RESTORE".bright_magenta(),
                label,
                previous_uid
            );

            match self.set_output_device(selector, label, previous_uid) {
                Ok(()) => info!(
                    "{} Successfully restored {} to '{}'",
                    "SYS_AUDIO_RESTORED".bright_green(),
                    label,
                    previous_uid
                ),
                Err(e) => warn!("{} {}", "SYS_AUDIO_WARN".bright_yellow(), e),
            }
        }

        SystemAudioStateService::reset_diversion(&self.db).await?;

        Ok(())
    }
}
