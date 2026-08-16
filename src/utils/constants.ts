// Application constants for the mixer interface

export const SampleRate = [44100, 48000, 88200, 96000] as const;
export type SampleRate = (typeof SampleRate)[number];

export const DEFAULT_SAMPLE_RATE_HZ: SampleRate = 48000;

// Audio processing constants
export const AUDIO = {
  // Sample rates
  SAMPLE_RATES: SampleRate,
  DEFAULT_SAMPLE_RATE: DEFAULT_SAMPLE_RATE_HZ,

  // Buffer sizes
  BUFFER_SIZES: [128, 256, 512, 1024, 2048] as const,
  DEFAULT_BUFFER_SIZE: 512,

  // Gain ranges
  MIN_GAIN_DB: -60,
  MAX_GAIN_DB: 12,
  DEFAULT_GAIN: 0,

  // Pan range
  MIN_PAN: -1,
  MAX_PAN: 1,
  DEFAULT_PAN: 0,

  // VU meter constants
  VU_MIN_DB: -60,
  VU_MAX_DB: 0,
  VU_UPDATE_RATE: 2000, // ms (optimized from 100ms to 50ms for smoother animation)
  VU_THROTTLE_RATE: 33, // ms (30fps throttle for rendering)
  VU_SEGMENTS: 30,

  // EQ defaults
  EQ_MIN_GAIN: -12,
  EQ_MAX_GAIN: 12,
  EQ_DEFAULT_GAIN: 0,

  // Compressor defaults
  COMP_MIN_THRESHOLD: -40,
  COMP_MAX_THRESHOLD: 0,
  COMP_DEFAULT_THRESHOLD: -12,
  COMP_MIN_RATIO: 1,
  COMP_MAX_RATIO: 10,
  COMP_DEFAULT_RATIO: 4,
  COMP_MIN_ATTACK: 0.1,
  COMP_MAX_ATTACK: 100,
  COMP_DEFAULT_ATTACK: 10,
  COMP_MIN_RELEASE: 10,
  COMP_MAX_RELEASE: 1000,
  COMP_DEFAULT_RELEASE: 100,

  // Limiter defaults
  LIMITER_MIN_THRESHOLD: -12,
  LIMITER_MAX_THRESHOLD: 0,
  LIMITER_DEFAULT_THRESHOLD: -3,
} as const;
