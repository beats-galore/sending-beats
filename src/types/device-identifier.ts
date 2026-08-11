import type { ConfiguredAudioDevice } from './db/configured-audio-devices.types';
import type { Identifier } from './util.types';

/**
 * The single boundary where an OS-provided device string becomes a device
 * identifier.
 *
 * Device IDs reach the interface two ways: already branded, from the database
 * (`ConfiguredAudioDevice.deviceIdentifier`), or as plain strings from Core
 * Audio enumeration and `<select>` values. Branding is erased at runtime, so
 * crossing that gap needs one assertion — kept here, named, rather than spread
 * across every component that reads a device out of a form control.
 */
export const asDeviceIdentifier = (id: string): Identifier<ConfiguredAudioDevice> =>
  id as Identifier<ConfiguredAudioDevice>;
