// Mixer-specific types for the virtual mixing console

export type AudioChannel = {
  id: number;
  name: string;
  input_device_id?: string;
  gain: number;
  pan: number;
  muted: boolean;
  solo: boolean;
  effects_enabled: boolean;
  peak_level: number;
  rms_level: number;
  
  // Stereo level data (optional for backward compatibility)
  peak_left?: number;
  rms_left?: number;
  peak_right?: number;
  rms_right?: number;

  // Effects (keeping original property names for Tauri compatibility)
  eq_low_gain: number;
  eq_mid_gain: number;
  eq_high_gain: number;

  comp_threshold: number;
  comp_ratio: number;
  comp_attack: number;
  comp_release: number;
  comp_enabled: boolean;

  limiter_threshold: number;
  limiter_enabled: boolean;
};

export type MixerConfig = {
  sample_rate: number;
  buffer_size: number;
  channels: AudioChannel[];
  master_gain: number;
  master_output_device_id?: string;
  monitor_output_device_id?: string;
  enable_loopback: boolean;
};

// Channel creation defaults
export const DEFAULT_CHANNEL: Omit<AudioChannel, 'id' | 'name'> = {
  input_device_id: undefined,
  gain: 0, // dB
  pan: 0, // center
  muted: false,
  solo: false,
  effects_enabled: false,
  peak_level: 0,
  rms_level: 0,

  // EQ defaults (flat response)
  eq_low_gain: 0,
  eq_mid_gain: 0,
  eq_high_gain: 0,

  // Compressor defaults (disabled)
  comp_threshold: -12,
  comp_ratio: 4,
  comp_attack: 10,
  comp_release: 100,
  comp_enabled: false,

  // Limiter defaults (disabled)
  limiter_threshold: -3,
  limiter_enabled: false,
} as const;

// Mixer state enums
export enum MixerState {
  STOPPED = 'stopped',
  STARTING = 'starting',
  RUNNING = 'running',
  STOPPING = 'stopping',
  ERROR = 'error',
}

// Channel operation types
export type ChannelUpdate = Partial<Omit<AudioChannel, 'id' | 'name' | 'peak_level' | 'rms_level'>>;

export type MixerOperationResult = {
  success: boolean;
  error?: string;
};
