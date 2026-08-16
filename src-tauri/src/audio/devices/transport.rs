// How a device is attached to the machine.
//
// The one thing that separates a piece of hardware from a piece of software
// pretending to be one. BlackHole, an aggregate, and our own HAL plugin all
// report themselves as virtual; a microphone, a Focusrite and a DJ controller
// report the bus they are plugged into.
//
// Read from CoreAudio rather than guessed from the name. Matching on "BlackHole"
// works until someone renames it, installs a driver nobody thought of, or plugs
// in an interface with an unlucky name — and the interface offering to add a
// "virtual input" has to be right about which ones those are.

use serde::{Deserialize, Serialize};

/// How a device reaches the machine, as CoreAudio reports it
///
/// Deliberately not every code Apple defines: what the interface needs to know
/// is whether something is real, and the rest is a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceTransport {
    BuiltIn,
    Usb,
    Bluetooth,
    Thunderbolt,
    FireWire,
    Pci,
    Hdmi,
    DisplayPort,
    AirPlay,
    Avb,
    /// Software presenting itself as a device — BlackHole, our own driver
    Virtual,
    /// Several real devices bound together, which is software doing the binding
    Aggregate,
    /// Reported as something this build does not have a name for
    Unknown,
}

impl DeviceTransport {
    /// Whether this is software rather than hardware
    ///
    /// An aggregate counts: it is a wrapper the user made, not something they
    /// plugged in, and it belongs with the other things that only exist because
    /// some software is running.
    pub fn is_virtual(self) -> bool {
        matches!(self, Self::Virtual | Self::Aggregate)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::DeviceTransport;
    use coreaudio_sys::{
        kAudioDevicePropertyTransportType, kAudioObjectPropertyElementMaster,
        kAudioObjectPropertyScopeGlobal, AudioDeviceID, AudioObjectPropertyAddress,
    };

    /// The four-character codes CoreAudio answers with
    ///
    /// Spelled out rather than taken from the bindings, which do not export
    /// every one of them, and because a literal here reads as the code Apple
    /// documents.
    const fn fourcc(code: &[u8; 4]) -> u32 {
        ((code[0] as u32) << 24)
            | ((code[1] as u32) << 16)
            | ((code[2] as u32) << 8)
            | code[3] as u32
    }

    /// Ask a device how it is attached
    ///
    /// Anything that fails — an old driver, a device that went away mid-call —
    /// comes back `Unknown` rather than an error. Not knowing how a device is
    /// attached is no reason to leave it out of the list.
    pub fn transport_of(device_id: AudioDeviceID) -> DeviceTransport {
        let address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyTransportType,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        let mut transport: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;

        let status = unsafe {
            coreaudio_sys::AudioObjectGetPropertyData(
                device_id,
                &address as *const _,
                0,
                std::ptr::null(),
                &mut size as *mut _,
                &mut transport as *mut _ as *mut _,
            )
        };

        if status != 0 {
            return DeviceTransport::Unknown;
        }

        match transport {
            t if t == fourcc(b"bltn") => DeviceTransport::BuiltIn,
            t if t == fourcc(b"usb ") => DeviceTransport::Usb,
            t if t == fourcc(b"blue") => DeviceTransport::Bluetooth,
            t if t == fourcc(b"thun") => DeviceTransport::Thunderbolt,
            t if t == fourcc(b"1394") => DeviceTransport::FireWire,
            t if t == fourcc(b"pci ") => DeviceTransport::Pci,
            t if t == fourcc(b"hdmi") => DeviceTransport::Hdmi,
            t if t == fourcc(b"dprt") => DeviceTransport::DisplayPort,
            t if t == fourcc(b"airp") => DeviceTransport::AirPlay,
            t if t == fourcc(b"eavb") => DeviceTransport::Avb,
            t if t == fourcc(b"virt") => DeviceTransport::Virtual,
            t if t == fourcc(b"grup") => DeviceTransport::Aggregate,
            _ => DeviceTransport::Unknown,
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::transport_of;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_devices_are_virtual() {
        assert!(DeviceTransport::Virtual.is_virtual());
        assert!(DeviceTransport::Aggregate.is_virtual());
    }

    #[test]
    fn everything_plugged_in_is_not() {
        for transport in [
            DeviceTransport::BuiltIn,
            DeviceTransport::Usb,
            DeviceTransport::Bluetooth,
            DeviceTransport::Thunderbolt,
            DeviceTransport::FireWire,
            DeviceTransport::Pci,
            DeviceTransport::Hdmi,
            DeviceTransport::DisplayPort,
            DeviceTransport::AirPlay,
            DeviceTransport::Avb,
        ] {
            assert!(
                !transport.is_virtual(),
                "{:?} should not be virtual",
                transport
            );
        }
    }

    /// An unreadable transport is treated as hardware
    ///
    /// The safer way round: a real device left out of "physical inputs" is a
    /// device the user cannot patch at all, where a virtual one appearing there
    /// is only untidy.
    #[test]
    fn unknown_is_not_virtual() {
        assert!(!DeviceTransport::Unknown.is_virtual());
    }
}
