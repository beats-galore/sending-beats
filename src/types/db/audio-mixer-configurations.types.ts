// Audio mixer configuration types
// Corresponds to src-tauri/src/db/audio_mixer_configurations.rs

import type { Timestamp, Uuid } from '../util.types';

export type AudioMixerConfiguration = {
  id: Uuid<AudioMixerConfiguration>; // UUID as string
  name: string;
  description?: string;
  configurationType: 'reusable' | 'session'; // Will be camelCase after serde conversion
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
  deletedAt?: Timestamp; // ISO timestamp, null for active records
};
