// What a device is doing underneath everything playing through it.
//
// A device carries its own mute and its own volume, and both sit below every
// application: a muted device silences a stream that is otherwise running
// perfectly, with no error and no missing frames. That is indistinguishable
// from a broken output unless somebody asks the device.
//
// It matters here because the mute and volume keys act on whichever device is
// the system output. Divert the system output to the virtual driver — which is
// what makes system audio capturable without doubling up — and whatever those
// keys did to the previous device stays done. Patch that device in as a mixer
// destination afterwards and it is silent for a reason nothing on screen shows.

use serde::{Deserialize, Serialize};

/// What a device says about its own audibility
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputHealth {
    /// Muted at the device, which silences every stream playing through it
    pub muted: bool,
    /// The device's own volume, 0.0 to 1.0, when it has one to report
    pub volume: Option<f32>,
    /// Whether the device reports these at all — an aggregate often does not
    pub reports_mute: bool,
    pub reports_volume: bool,
}

impl OutputHealth {
    /// Whether this device would swallow whatever is played into it
    ///
    /// A device that reports nothing is not silent: plenty of perfectly working
    /// outputs have no mute or volume of their own, and treating "did not say"
    /// as "muted" would accuse every one of them.
    pub fn is_silenced(&self) -> bool {
        self.muted || self.volume.is_some_and(|level| level <= 0.0)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::OutputHealth;
    use coreaudio_sys::{
        kAudioDevicePropertyMute, kAudioDevicePropertyVolumeScalar,
        kAudioObjectPropertyElementMaster, kAudioObjectPropertyScopeOutput, AudioDeviceID,
        AudioObjectPropertyAddress,
    };

    /// The master element, and then the first two channels
    ///
    /// A device may carry its volume per channel rather than on the master —
    /// the built-in speakers do — so asking only the master reports nothing on
    /// exactly the device this exists to ask about.
    const ELEMENTS: [u32; 3] = [kAudioObjectPropertyElementMaster, 1, 2];

    fn address(selector: u32, element: u32) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress {
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeOutput,
            mElement: element,
        }
    }

    fn read<T: Copy + Default>(device_id: AudioDeviceID, selector: u32, element: u32) -> Option<T> {
        let address = address(selector, element);

        // Asked first, because plenty of devices simply do not carry a mute or
        // a volume and reading one that is not there is an error rather than a
        // value worth reporting.
        let has = unsafe { coreaudio_sys::AudioObjectHasProperty(device_id, &address as *const _) };
        if has == 0 {
            return None;
        }

        let mut value = T::default();
        let mut size = std::mem::size_of::<T>() as u32;

        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                device_id,
                &address as *const _,
                0,
                std::ptr::null(),
                &mut size as *mut _,
                &mut value as *mut _ as *mut _,
            )
        };

        (status == 0).then_some(value)
    }

    /// Take a device off mute, on every element that carries one
    ///
    /// The studio owns a device it is playing through. A device mute sits below
    /// every stream and is set by keys that may not even be pointed at this
    /// device any more, so it is not a setting the mixer can honour — it is a
    /// state that has to be cleared for the destination to mean anything.
    ///
    /// Not put back afterwards. Handing back a muted device is how this went
    /// wrong in the first place: something is left silent with nothing on
    /// screen to say so, and unmuted is the recoverable direction.
    pub fn clear_mute(device_id: AudioDeviceID) -> bool {
        let mut cleared = false;

        for element in ELEMENTS {
            let address = address(kAudioDevicePropertyMute, element);

            let has =
                unsafe { coreaudio_sys::AudioObjectHasProperty(device_id, &address as *const _) };
            if has == 0 {
                continue;
            }

            let off: u32 = 0;
            let status = unsafe {
                coreaudio_sys::AudioObjectSetPropertyData(
                    device_id,
                    &address as *const _,
                    0,
                    std::ptr::null(),
                    std::mem::size_of::<u32>() as u32,
                    &off as *const _ as *const _,
                )
            };

            cleared = cleared || status == 0;
        }

        cleared
    }

    /// Ask a device whether it would let anything through
    pub fn output_health(device_id: AudioDeviceID) -> OutputHealth {
        let mute = ELEMENTS
            .iter()
            .find_map(|element| read::<u32>(device_id, kAudioDevicePropertyMute, *element));

        // The quietest channel is the answer: one channel at zero is half a
        // stereo image gone, which is a fault worth naming even though the
        // other side is audible.
        let volume = ELEMENTS
            .iter()
            .filter_map(|element| {
                read::<f32>(device_id, kAudioDevicePropertyVolumeScalar, *element)
            })
            .fold(None, |lowest: Option<f32>, level| {
                Some(lowest.map_or(level, |current| current.min(level)))
            });

        OutputHealth {
            muted: mute.is_some_and(|value| value != 0),
            volume,
            reports_mute: mute.is_some(),
            reports_volume: volume.is_some(),
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{clear_mute, output_health};

#[cfg(test)]
mod tests {
    use super::OutputHealth;

    fn health(muted: bool, volume: Option<f32>) -> OutputHealth {
        OutputHealth {
            muted,
            volume,
            reports_mute: true,
            reports_volume: volume.is_some(),
        }
    }

    #[test]
    fn a_muted_device_swallows_everything() {
        assert!(health(true, Some(1.0)).is_silenced());
    }

    #[test]
    fn so_does_one_turned_all_the_way_down() {
        assert!(health(false, Some(0.0)).is_silenced());
    }

    #[test]
    fn an_audible_device_is_not_silenced() {
        assert!(!health(false, Some(0.5)).is_silenced());
    }

    /// Plenty of working outputs carry no mute or volume of their own
    #[test]
    fn saying_nothing_is_not_an_accusation() {
        let quiet = OutputHealth {
            muted: false,
            volume: None,
            reports_mute: false,
            reports_volume: false,
        };

        assert!(!quiet.is_silenced());
    }
}
