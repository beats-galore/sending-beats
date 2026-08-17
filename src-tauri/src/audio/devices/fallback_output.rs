// Somewhere real to send audio back to.
//
// Diversion saves the default output so it can be put back afterwards. If what
// it saved is our own virtual device — which is what it was diverting away
// from, and what the machine reports as the default for as long as diversion
// lasts — then undiverting sets the default to a loopback with nothing feeding
// it. The machine goes quiet and the only way out is Control Centre.
//
// So the saved device can never be ours, and when there is nothing valid to go
// back to there has to be an answer anyway. The built-in output is that answer:
// it is the one output every Mac has, it is what the machine came set to, and
// it cannot be unplugged.

use crate::audio::devices::transport::{transport_of, DeviceTransport};

#[cfg(target_os = "macos")]
use coreaudio_sys::{
    kAudioDevicePropertyStreams, kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMaster,
    kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject,
    AudioDeviceID, AudioObjectPropertyAddress,
};

/// An output device and how it is attached
#[derive(Debug, Clone, PartialEq)]
pub struct OutputCandidate {
    pub device_id: u32,
    pub uid: String,
    pub transport: DeviceTransport,
}

/// Which device to hand the system back to
///
/// Preference order, and why:
///
/// 1. What was saved, as long as it is neither ours nor gone. Putting a machine
///    back where it was is the whole point.
/// 2. The first built-in output. Every Mac has one, it is what the machine came
///    set to, and it cannot be unplugged between diverting and undiverting.
/// 3. Any output at all. A machine pointed at something is recoverable; one
///    pointed at nothing sounds broken.
pub fn choose_restore_target(
    saved: Option<&str>,
    ours: &str,
    available: &[OutputCandidate],
) -> Option<String> {
    if let Some(saved) = saved {
        let usable = saved != ours && available.iter().any(|entry| entry.uid == saved);
        if usable {
            return Some(saved.to_string());
        }
    }

    available
        .iter()
        .find(|entry| entry.uid != ours && entry.transport == DeviceTransport::BuiltIn)
        .or_else(|| available.iter().find(|entry| entry.uid != ours))
        .map(|entry| entry.uid.clone())
}

/// Whether a device is one we should ever record as the previous default
///
/// Ours never is. It is the thing being diverted to, so saving it is saving the
/// state we are trying to be able to undo.
pub fn is_worth_saving(uid: &str, ours: &str) -> bool {
    uid != ours
}

#[cfg(target_os = "macos")]
/// Every device that can play audio, with how it is attached
pub fn output_candidates(uid_of: impl Fn(AudioDeviceID) -> Option<String>) -> Vec<OutputCandidate> {
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    let mut size: u32 = 0;
    let status = unsafe {
        coreaudio_sys::AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject,
            &address as *const _,
            0,
            std::ptr::null(),
            &mut size as *mut _,
        )
    };

    if status != 0 || size == 0 {
        return Vec::new();
    }

    let count = size as usize / std::mem::size_of::<AudioDeviceID>();
    let mut ids = vec![0 as AudioDeviceID; count];

    let status = unsafe {
        coreaudio_sys::AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &address as *const _,
            0,
            std::ptr::null(),
            &mut size as *mut _,
            ids.as_mut_ptr() as *mut _,
        )
    };

    if status != 0 {
        return Vec::new();
    }

    ids.into_iter()
        .filter(|id| has_output_streams(*id))
        .filter_map(|id| {
            uid_of(id).map(|uid| OutputCandidate {
                device_id: id,
                uid,
                transport: transport_of(id),
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
/// Whether a device can actually play anything
///
/// An input-only device is still a device, and handing the system's output to
/// a microphone is worse than handing it to nothing.
fn has_output_streams(device_id: AudioDeviceID) -> bool {
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyStreams,
        mScope: kAudioObjectPropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMaster,
    };

    let mut size: u32 = 0;
    let status = unsafe {
        coreaudio_sys::AudioObjectGetPropertyDataSize(
            device_id,
            &address as *const _,
            0,
            std::ptr::null(),
            &mut size as *mut _,
        )
    };

    status == 0 && size > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const OURS: &str = "SweetBeatsStudio2ch_UID";

    fn candidate(uid: &str, transport: DeviceTransport) -> OutputCandidate {
        OutputCandidate {
            device_id: 0,
            uid: uid.to_string(),
            transport,
        }
    }

    fn machine() -> Vec<OutputCandidate> {
        vec![
            candidate("BlackHole2ch_UID", DeviceTransport::Virtual),
            candidate(OURS, DeviceTransport::Virtual),
            candidate("BuiltInSpeakerDevice", DeviceTransport::BuiltIn),
            candidate("AggregateDevice-1", DeviceTransport::Aggregate),
        ]
    }

    #[test]
    fn what_was_saved_is_what_comes_back() {
        let chosen = choose_restore_target(Some("BuiltInSpeakerDevice"), OURS, &machine());
        assert_eq!(chosen.as_deref(), Some("BuiltInSpeakerDevice"));
    }

    /// The bug: our own device recorded as the thing to go back to
    #[test]
    fn our_own_device_is_never_restored_to() {
        let chosen = choose_restore_target(Some(OURS), OURS, &machine());

        assert_eq!(
            chosen.as_deref(),
            Some("BuiltInSpeakerDevice"),
            "falls through to the built-in rather than handing back a loopback"
        );
    }

    #[test]
    fn a_saved_device_that_has_gone_falls_back() {
        let chosen = choose_restore_target(Some("UnpluggedInterface"), OURS, &machine());
        assert_eq!(chosen.as_deref(), Some("BuiltInSpeakerDevice"));
    }

    #[test]
    fn nothing_saved_falls_back_to_the_built_in() {
        assert_eq!(
            choose_restore_target(None, OURS, &machine()).as_deref(),
            Some("BuiltInSpeakerDevice")
        );
    }

    /// A machine with no built-in output still has to be given something
    #[test]
    fn with_no_built_in_the_first_output_will_do() {
        let no_built_in = vec![
            candidate(OURS, DeviceTransport::Virtual),
            candidate("BlackHole2ch_UID", DeviceTransport::Virtual),
            candidate("UsbInterface", DeviceTransport::Usb),
        ];

        assert_eq!(
            choose_restore_target(None, OURS, &no_built_in).as_deref(),
            Some("BlackHole2ch_UID"),
            "the first that is not ours, in the order the system lists them"
        );
    }

    /// A machine whose only output is ours has nowhere to go, and says so
    #[test]
    fn with_nothing_but_ours_there_is_no_answer() {
        let only_ours = vec![candidate(OURS, DeviceTransport::Virtual)];
        assert_eq!(choose_restore_target(Some(OURS), OURS, &only_ours), None);
    }

    #[test]
    fn our_device_is_never_worth_saving() {
        assert!(!is_worth_saving(OURS, OURS));
        assert!(is_worth_saving("BuiltInSpeakerDevice", OURS));
    }
}
