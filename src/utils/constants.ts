// Application constants for the mixer interface

// Audio processing constants
export const AUDIO = {
  // Sample rates
  SAMPLE_RATES: [44100, 48000, 88200, 96000] as const,
  DEFAULT_SAMPLE_RATE: 44100,

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
  VU_UPDATE_RATE: 50, // ms (optimized from 100ms to 50ms for smoother animation)
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

// UI constants
export const UI = {
  // Responsive breakpoints
  BREAKPOINTS: {
    SM: 640,
    MD: 768,
    LG: 1024,
    XL: 1280,
    '2XL': 1536,
  },

  // Animation durations
  ANIMATION: {
    FAST: 150,
    NORMAL: 300,
    SLOW: 500,
  },

  // Polling intervals
  POLLING: {
    VU_METERS: 100,
    DEVICE_REFRESH: 5000,
    METRICS: 1000,
  },

  // Component sizes
  CHANNEL_WIDTH: 120,
  VU_METER_HEIGHT: 200,
  SLIDER_HEIGHT: 150,
} as const;

// Color constants
export const COLORS = {
  // VU meter colors
  VU_METER: {
    BACKGROUND: '#374151',
    OFF: '#4b5563',
    GREEN: '#10b981',
    YELLOW: '#f59e0b',
    RED: '#ef4444',
    // Thresholds (0-1 range)
    GREEN_THRESHOLD: 0.7,
    YELLOW_THRESHOLD: 0.85,
    RED_THRESHOLD: 1.0,
  },

  // Button states
  BUTTON: {
    DEFAULT: '#6b7280',
    ACTIVE: '#3b82f6',
    MUTED: '#ef4444',
    SOLO: '#f59e0b',
    SUCCESS: '#10b981',
    WARNING: '#f59e0b',
    DANGER: '#ef4444',
  },
} as const;

// Keyboard shortcuts
export const KEYBOARD = {
  PLAY_PAUSE: ' ',
  MUTE_ALL: 'm',
  SOLO_CLEAR: 's',
  MASTER_GAIN_UP: 'ArrowUp',
  MASTER_GAIN_DOWN: 'ArrowDown',
} as const;

// Error messages
export const ERRORS = {
  MIXER_NOT_INITIALIZED: 'Mixer not initialized',
  DEVICE_NOT_FOUND: 'Audio device not found',
  STREAM_CREATE_FAILED: 'Failed to create audio stream',
  CHANNEL_UPDATE_FAILED: 'Failed to update channel',
  PERMISSION_DENIED: 'Audio permission denied',
  DEVICE_BUSY: 'Audio device is busy',
} as const;

// Default channel configuration
export const DEFAULT_CHANNEL_CONFIG = {
  GAIN: AUDIO.DEFAULT_GAIN,
  PAN: AUDIO.DEFAULT_PAN,
  MUTED: false,
  SOLO: false,
  EFFECTS_ENABLED: false,

  // EQ
  EQ_LOW_GAIN: AUDIO.EQ_DEFAULT_GAIN,
  EQ_MID_GAIN: AUDIO.EQ_DEFAULT_GAIN,
  EQ_HIGH_GAIN: AUDIO.EQ_DEFAULT_GAIN,

  // Compressor
  COMP_THRESHOLD: AUDIO.COMP_DEFAULT_THRESHOLD,
  COMP_RATIO: AUDIO.COMP_DEFAULT_RATIO,
  COMP_ATTACK: AUDIO.COMP_DEFAULT_ATTACK,
  COMP_RELEASE: AUDIO.COMP_DEFAULT_RELEASE,
  COMP_ENABLED: false,

  // Limiter
  LIMITER_THRESHOLD: AUDIO.LIMITER_DEFAULT_THRESHOLD,
  LIMITER_ENABLED: false,
} as const;
