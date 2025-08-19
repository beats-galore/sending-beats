// UI component types and props types
import { AudioChannel, MixerConfig } from './mixer.types';
import { AudioDeviceInfo, MasterLevels, AudioMetrics } from './audio.types';

// VU Meter component types
export type VUMeterProps = {
  peakLevel: number;
  rmsLevel: number;
  vertical?: boolean;
  height?: number;
  width?: number;
  showLabels?: boolean;
};

// Channel strip component types
export type ChannelStripProps = {
  channel: AudioChannel;
  inputDevices: AudioDeviceInfo[];
  onChannelUpdate: (channelId: number, updates: Partial<AudioChannel>) => void;
};

// Master section component types
export type MasterSectionProps = {
  mixerConfig: MixerConfig | null;
  masterLevels: MasterLevels;
  metrics: AudioMetrics | null;
  outputDevices: AudioDeviceInfo[];
  onMasterGainChange: (gain: number) => void;
  onOutputDeviceChange: (deviceId: string) => void;
};

// Audio effects component types
export type CompressorProps = {
  threshold: number;
  ratio: number;
  attack: number;
  release: number;
  enabled: boolean;
  onUpdate: (updates: {
    comp_threshold?: number;
    comp_ratio?: number;
    comp_attack?: number;
    comp_release?: number;
    comp_enabled?: boolean;
  }) => void;
};

export type ThreeBandEQProps = {
  lowGain: number;
  midGain: number;
  highGain: number;
  onUpdate: (updates: {
    eq_low_gain?: number;
    eq_mid_gain?: number;
    eq_high_gain?: number;
  }) => void;
};

export type LimiterProps = {
  threshold: number;
  enabled: boolean;
  onUpdate: (updates: {
    limiter_threshold?: number;
    limiter_enabled?: boolean;
  }) => void;
};

// UI control component types
export type AudioSliderProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  onChange: (value: number) => void;
  disabled?: boolean;
};

export type ToggleButtonProps = {
  label: string;
  pressed: boolean;
  onChange: (pressed: boolean) => void;
  variant?: 'default' | 'success' | 'warning' | 'danger';
  disabled?: boolean;
};

export type DeviceSelectorProps = {
  devices: AudioDeviceInfo[];
  selectedDeviceId?: string;
  onDeviceChange: (deviceId: string) => void;
  placeholder?: string;
  disabled?: boolean;
};

// Loading and error states
export type LoadingSpinnerProps = {
  size?: 'sm' | 'md' | 'lg';
  color?: string;
};

// Color scheme for VU meters
export const VU_METER_COLORS = {
  BACKGROUND: '#374151', // gray-700
  GREEN: '#10b981',      // emerald-500
  YELLOW: '#f59e0b',     // amber-500
  RED: '#ef4444',        // red-500
  OFF: '#4b5563',        // gray-600
} as const;

// Breakpoints for VU meter color zones
export const VU_METER_ZONES = {
  GREEN_THRESHOLD: 0.7,   // 70% = -18dB
  YELLOW_THRESHOLD: 0.85, // 85% = -9dB
  RED_THRESHOLD: 1.0,     // 100% = 0dB
} as const;

// Button colors
export const COLORS = {
  BUTTON: {
    DEFAULT: '#6b7280',
    ACTIVE: '#3b82f6',
    MUTED: '#ef4444',
    SOLO: '#f59e0b',
    SUCCESS: '#10b981',
    WARNING: '#f59e0b',
    DANGER: '#ef4444'
  }
} as const;