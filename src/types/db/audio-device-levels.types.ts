// Audio device levels types (VU meter data)
// Corresponds to src-tauri/src/db/audio_device_levels.rs

import type { ConfiguredAudioDevice } from './configured-audio-devices.types';
import type { Timestamp, Uuid } from '../util.types';

export type VULevelData = {
  id: Uuid<VULevelData>; // UUID as string
  audioDeviceId: Uuid<ConfiguredAudioDevice>; // UUID as string
  configurationId: string; // UUID as string
  peakLeft: number; // Peak level for left channel (-∞ to 0 dB)
  peakRight: number; // Peak level for right channel (-∞ to 0 dB)
  rmsLeft: number; // RMS level for left channel (-∞ to 0 dB)
  rmsRight: number; // RMS level for right channel (-∞ to 0 dB)
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
  deletedAt?: Timestamp; // ISO timestamp
};

// Simplified VU level data for real-time processing (backwards compatibility)
export type SimplifiedVULevelData = {
  timestamp: number; // Microseconds since Unix epoch
  channelId: number;
  peakLeft: number;
  rmsLeft: number;
  peakRight?: number; // None for mono sources
  rmsRight?: number; // None for mono sources
  isStereo: boolean;
};

// VU meter display data
export type VUMeterLevels = {
  peakLeft: number;
  peakRight: number;
  rmsLeft: number;
  rmsRight: number;
  timestamp: number;
};

// Channel levels for real-time display
export type ChannelLevels = Record<number, VUMeterLevels>;
