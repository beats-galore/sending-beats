// Audio effects types
// Corresponds to src-tauri/src/db/audio_effects.rs

import type { Timestamp, Uuid } from '../util.types';
import type { AudioMixerConfiguration } from './audio-mixer-configurations.types';
import type { ConfiguredAudioDevice } from './configured-audio-devices.types';

export type AudioEffectsDefault = {
  id: Uuid<AudioEffectsDefault>; // UUID as string
  deviceId: Uuid<ConfiguredAudioDevice>; // UUID as string
  configurationId: Uuid<AudioMixerConfiguration>; // UUID as string
  gain: number; // in dB
  pan: number; // -1.0 (left) to 1.0 (right)
  muted: boolean;
  solo: boolean;
  eqLowGain: number; // dB
  eqMidGain: number; // dB
  eqHighGain: number; // dB
  compThreshold: number; // dB
  compRatio: number; // 1.0 and up
  compAttack: number; // ms
  compRelease: number; // ms
  compEnabled: boolean;
  limiterThreshold: number; // dB
  limiterEnabled: boolean;
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
};

/** The EQ bands a single update may carry; omitted bands keep their value. */
export type EqBandUpdate = {
  lowGain?: number;
  midGain?: number;
  highGain?: number;
};

/** Compressor settings a single update may carry. */
export type CompressorUpdate = {
  threshold?: number;
  ratio?: number;
  attack?: number;
  release?: number;
  enabled?: boolean;
};

/** Limiter settings a single update may carry. */
export type LimiterUpdate = {
  threshold?: number;
  enabled?: boolean;
};

export const AudioEffectType = ['equalizer', 'limiter', 'compressor'] as const;
export type AudioEffectType = (typeof AudioEffectType)[number];

// Common effect parameter types for type safety
export type EqualizerParameters = {
  lowGain: number; // dB
  midGain: number; // dB
  highGain: number; // dB
  lowFreq?: number; // Hz
  midFreq?: number; // Hz
  highFreq?: number; // Hz
  lowQ?: number;
  midQ?: number;
  highQ?: number;
};

export type CompressorParameters = {
  threshold: number; // dB
  ratio: number; // 1.0 to inf
  attack: number; // ms
  release: number; // ms
  makeupGain?: number; // dB
  knee?: number; // dB
};

export type LimiterParameters = {
  threshold: number; // dB
  lookahead?: number; // ms
  release?: number; // ms
};

type AudioEffectParameterMap = {
  limiter: LimiterParameters;
  equalizer: EqualizerParameters;
  compressor: CompressorParameters;
};

export type AudioEffectParameters<T extends AudioEffectType> = AudioEffectParameterMap[T];

export type AudioEffectsCustom<T extends AudioEffectType = AudioEffectType> = {
  id: string; // UUID as string
  deviceId: string; // UUID as string
  configurationId: string; // UUID as string
  effectType: T; // 'equalizer', 'compressor', 'limiter', etc.
  parameters: AudioEffectParameters<T>;
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
};
