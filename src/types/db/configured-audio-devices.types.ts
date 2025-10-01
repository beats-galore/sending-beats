// Configured audio devices types
// Corresponds to src-tauri/src/db/configured_audio_devices.rs

import type { Identifier, Timestamp, Uuid } from '../util.types';
import type { AsCreationAttributes } from './util';

export type ConfiguredAudioDevice = {
  id: Uuid<ConfiguredAudioDevice>; // UUID as string
  deviceIdentifier: Identifier<ConfiguredAudioDevice>;
  deviceName?: string;
  sampleRate: number;
  bufferSize?: number;
  channelFormat: 'stereo' | 'mono';
  isVirtual: boolean;
  isInput: boolean;
  configurationId: Uuid<AudioConfiguration>; // UUID as string
  channelNumber: number;
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
  deletedAt?: Timestamp; // ISO timestamp
};

export type CreateConfiguredAudioDevice = AsCreationAttributes<ConfiguredAudioDevice>;

export type UpdateConfiguredAudioDevice = Partial<
  Omit<CreateConfiguredAudioDevice, 'configurationId' | 'isInput'>
>;
