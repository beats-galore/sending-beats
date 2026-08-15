// Events the backend device watcher emits when Core Audio reports a hardware
// change. Names must match `src-tauri/src/audio/devices/device_watcher.rs`.

export const DEVICES_CHANGED_EVENT = 'audio-devices-changed';
export const DEVICE_DISCONNECTED_EVENT = 'audio-device-disconnected';
export const DEVICE_RECONNECTED_EVENT = 'audio-device-reconnected';

/**
 * A device the mixer was using has gone away. The backend has already torn its
 * stream down; this exists so the UI can mark whatever was bound to it.
 */
export type DeviceDisconnectedEvent = {
  deviceId: string;
  deviceName: string;
  isInput: boolean;
};

/**
 * A device with a saved channel binding came back. The backend has already
 * tried to rebuild its stream; `restored` says whether that worked.
 */
export type DeviceReconnectedEvent = {
  deviceId: string;
  deviceName: string;
  channelNumber: number;
  restored: boolean;
};
