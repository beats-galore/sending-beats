// Push notifications from the Core Audio HAL about devices arriving, leaving,
// and being promoted to system default.
//
// Core Audio delivers these on its own thread, so the listener callback does
// nothing but hand the event to a channel. Every reaction - re-enumeration,
// health updates, mixer teardown - happens on the consumer side.

use anyhow::{anyhow, Result};
use colored::Colorize;
use coreaudio_sys::{
    kAudioDevicePropertyDeviceHasChanged, kAudioDevicePropertyDeviceIsAlive,
    kAudioHardwarePropertyDefaultInputDevice, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMaster,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, AudioObjectAddPropertyListener,
    AudioObjectID, AudioObjectPropertyAddress, AudioObjectPropertySelector,
    AudioObjectRemovePropertyListener, OSStatus, UInt32,
};
use std::os::raw::c_void;
use std::sync::OnceLock;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tracing::{info, warn};

pub const LOG_TAG: &str = "DEVICE_HOTPLUG";

/// A hardware change the HAL told us about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEvent {
    /// A device appeared or disappeared. Carries no identity - the HAL only
    /// says the set changed, so the consumer has to re-enumerate and diff.
    DeviceListChanged,
    DefaultInputChanged,
    DefaultOutputChanged,
    /// A device we are watching individually stopped being alive.
    DeviceDied(AudioObjectID),
    /// A watched device's stream format or channel layout changed under us.
    DeviceConfigChanged(AudioObjectID),
}

/// Set once, read from the HAL callback thread. A global keeps the callback
/// from having to own a raw pointer back into Rust state, which is the usual
/// way this API turns into a use-after-free.
static EVENT_SENDER: OnceLock<UnboundedSender<DeviceEvent>> = OnceLock::new();

/// The system-wide properties worth watching, all on `kAudioObjectSystemObject`.
const SYSTEM_SELECTORS: [AudioObjectPropertySelector; 3] = [
    kAudioHardwarePropertyDevices,
    kAudioHardwarePropertyDefaultInputDevice,
    kAudioHardwarePropertyDefaultOutputDevice,
];

/// The per-device properties worth watching, registered per open device.
const DEVICE_SELECTORS: [AudioObjectPropertySelector; 2] = [
    kAudioDevicePropertyDeviceIsAlive,
    kAudioDevicePropertyDeviceHasChanged,
];

fn global_address(selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    }
}

/// Compared rather than matched: these constants are lowercase, so in pattern
/// position a dropped import would turn an arm into a catch-all binding that
/// silently swallows every selector.
fn classify(
    object_id: AudioObjectID,
    selector: AudioObjectPropertySelector,
) -> Option<DeviceEvent> {
    if selector == kAudioHardwarePropertyDevices {
        Some(DeviceEvent::DeviceListChanged)
    } else if selector == kAudioHardwarePropertyDefaultInputDevice {
        Some(DeviceEvent::DefaultInputChanged)
    } else if selector == kAudioHardwarePropertyDefaultOutputDevice {
        Some(DeviceEvent::DefaultOutputChanged)
    } else if selector == kAudioDevicePropertyDeviceIsAlive {
        Some(DeviceEvent::DeviceDied(object_id))
    } else if selector == kAudioDevicePropertyDeviceHasChanged {
        Some(DeviceEvent::DeviceConfigChanged(object_id))
    } else {
        None
    }
}

/// Called by Core Audio on a HAL-owned thread.
///
/// The HAL batches: one call can carry addresses for properties this listener
/// never asked about, so every address gets classified rather than assumed.
/// Nothing here may block, allocate unboundedly, or panic - it is `extern "C"`,
/// so unwinding past it is undefined behaviour.
unsafe extern "C" fn property_listener(
    object_id: AudioObjectID,
    num_addresses: UInt32,
    addresses: *const AudioObjectPropertyAddress,
    _client_data: *mut c_void,
) -> OSStatus {
    let (Some(sender), false) = (EVENT_SENDER.get(), addresses.is_null()) else {
        return 0;
    };

    let addresses = std::slice::from_raw_parts(addresses, num_addresses as usize);
    for address in addresses {
        if let Some(event) = classify(object_id, address.mSelector) {
            // A closed receiver means shutdown is underway; dropping is right.
            let _ = sender.send(event);
        }
    }

    0
}

/// Owns every listener registration so they come back off on drop. A listener
/// left registered past its client's lifetime is a crash waiting for the next
/// device change.
pub struct DevicePropertyListener {
    registrations: Vec<(AudioObjectID, AudioObjectPropertySelector)>,
}

impl DevicePropertyListener {
    /// Register for system-wide device changes and hand back the event stream.
    ///
    /// Only callable once per process - the sender is global, so a second call
    /// would orphan the first receiver.
    pub fn start() -> Result<(Self, UnboundedReceiver<DeviceEvent>)> {
        let (sender, receiver) = unbounded_channel();
        EVENT_SENDER
            .set(sender)
            .map_err(|_| anyhow!("Core Audio property listener already started"))?;

        let mut listener = Self {
            registrations: Vec::new(),
        };

        for selector in SYSTEM_SELECTORS {
            listener.register(kAudioObjectSystemObject, selector)?;
        }

        info!(
            "{} Listening for device arrival, removal, and default changes",
            LOG_TAG.on_cyan().black()
        );

        Ok((listener, receiver))
    }

    /// Watch one device for death and format changes.
    ///
    /// A device that dies while still listed - an interface losing power rather
    /// than being unplugged - never shows up in a device-list change, so open
    /// devices need this on top of the system-wide registration.
    pub fn watch_device(&mut self, device_id: AudioObjectID) -> Result<()> {
        for selector in DEVICE_SELECTORS {
            self.register(device_id, selector)?;
        }
        Ok(())
    }

    /// Stop watching one device, leaving the system-wide registrations alone.
    pub fn unwatch_device(&mut self, device_id: AudioObjectID) {
        self.registrations.retain(|(object_id, selector)| {
            if *object_id != device_id {
                return true;
            }
            remove_listener(*object_id, *selector);
            false
        });
    }

    fn register(
        &mut self,
        object_id: AudioObjectID,
        selector: AudioObjectPropertySelector,
    ) -> Result<()> {
        if self.registrations.contains(&(object_id, selector)) {
            return Ok(());
        }

        let address = global_address(selector);
        let status = unsafe {
            AudioObjectAddPropertyListener(
                object_id,
                &address as *const _,
                Some(property_listener),
                std::ptr::null_mut(),
            )
        };

        if status != 0 {
            return Err(anyhow!(
                "AudioObjectAddPropertyListener failed for object {} selector {:#x}: OSStatus {}",
                object_id,
                selector,
                status
            ));
        }

        self.registrations.push((object_id, selector));
        Ok(())
    }
}

fn remove_listener(object_id: AudioObjectID, selector: AudioObjectPropertySelector) {
    let address = global_address(selector);
    let status = unsafe {
        AudioObjectRemovePropertyListener(
            object_id,
            &address as *const _,
            Some(property_listener),
            std::ptr::null_mut(),
        )
    };

    if status != 0 {
        warn!(
            "{} Failed to remove listener for object {} selector {:#x}: OSStatus {}",
            LOG_TAG.on_cyan().black(),
            object_id,
            selector,
            status
        );
    }
}

impl Drop for DevicePropertyListener {
    fn drop(&mut self) {
        for (object_id, selector) in self.registrations.drain(..) {
            remove_listener(object_id, selector);
        }
    }
}

impl std::fmt::Debug for DevicePropertyListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DevicePropertyListener")
            .field("registrations", &self.registrations.len())
            .finish()
    }
}
