// Audio mixer configuration types
// Corresponds to src-tauri/src/db/audio_mixer_configurations.rs

import type { Timestamp, Uuid } from '../util.types';
import type { AsCreationAttributes, AsUpdateAttributes } from './util';

export type AudioMixerConfiguration = {
  id: Uuid<AudioMixerConfiguration>; // UUID as string
  name: string;
  description?: string;
  configurationType: 'reusable' | 'session'; // Will be camelCase after serde conversion
  sessionActive: boolean; // Only one configuration can be active at a time
  reusableConfigurationId?: Uuid<AudioMixerConfiguration>; // Self-referential for session configs
  isDefault: boolean; // Only one reusable configuration can be default
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
  deletedAt?: Timestamp; // ISO timestamp, null for active records
};

export type CreateAudioMixerConfiguration = AsCreationAttributes<AudioMixerConfiguration>;

export type UpdateAudioMixerConfiguration = AsUpdateAttributes<AudioMixerConfiguration>;
