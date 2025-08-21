// Core audio types for the Sendin Beats application
export type AudioDeviceInfo = {
  id: string;
  name: string;
  is_input: boolean;
  is_output: boolean;
  is_default: boolean;
  supported_sample_rates: number[];
  supported_channels: number[];
  host_api: string;
};

export type OutputDevice = {
  device_id: string;
  device_name: string;
  gain: number; // Individual output gain (0.0 - 2.0)
  enabled: boolean; // Whether this output is active
  is_monitor: boolean; // Whether this is a monitor/headphone output
};

export type DeviceStatus = 'Connected' | 'Disconnected' | { Error: string };

export type DeviceHealth = {
  device_id: string;
  device_name: string;
  status: DeviceStatus;
  last_seen: number; // timestamp
  error_count: number;
  consecutive_errors: number;
};

export type AudioMetrics = {
  cpu_usage: number;
  buffer_underruns: number;
  buffer_overruns: number;
  latency_ms: number;
  sample_rate: number;
  active_channels: number;
};

// Audio level data types
export type AudioLevels = {
  peak_level: number;
  rms_level: number;
};

export type MasterLevels = {
  left: AudioLevels;
  right: AudioLevels;
};

export type ChannelLevels = Record<number, [number, number, number, number]>; // [peak_left, rms_left, peak_right, rms_right]

// Audio processing types
export type ThreeBandEqualizer = {
  low_gain: number; // dB (-12 to +12)
  mid_gain: number; // dB (-12 to +12)
  high_gain: number; // dB (-12 to +12)
  enabled: boolean;
};

export type Compressor = {
  threshold: number; // dB (-40 to 0)
  ratio: number; // 1.0 to 10.0
  attack: number; // ms (0.1 to 100)
  release: number; // ms (10 to 1000)
  enabled: boolean;
};

export type Limiter = {
  threshold: number; // dB (-12 to 0)
  enabled: boolean;
};

// Audio processing constants
export const AUDIO_CONSTANTS = {
  MIN_DB: -60,
  MAX_DB: 0,
  MIN_GAIN_DB: -12,
  MAX_GAIN_DB: 12,
  MIN_PAN: -1,
  MAX_PAN: 1,
  DEFAULT_SAMPLE_RATE: 44100,
  DEFAULT_BUFFER_SIZE: 512,
  VU_UPDATE_INTERVAL: 100, // ms
} as const;
