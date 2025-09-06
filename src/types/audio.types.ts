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
  DEFAULT_SAMPLE_RATE: 48000,
  DEFAULT_BUFFER_SIZE: 512,
  VU_UPDATE_INTERVAL: 100, // ms
} as const;

// Recording types to match backend
export type RecordingMetadata = {
  // Core fields
  title?: string;
  artist?: string;
  album?: string;
  genre?: string;
  comment?: string;
  year?: number;

  // Extended fields
  album_artist?: string;
  composer?: string;
  track_number?: number;
  total_tracks?: number;
  disc_number?: number;
  total_discs?: number;
  copyright?: string;
  bpm?: number;
  isrc?: string;

  // Technical fields (auto-populated)
  encoder?: string;
  encoding_date?: string;
  sample_rate?: number;
  bitrate?: number;
  duration_seconds?: number;

  // Artwork
  artwork?: AlbumArtwork;

  // Custom fields
  custom_tags?: Record<string, string>;
};

export type AlbumArtwork = {
  mime_type: string;
  description: string;
  image_data: number[]; // Vec<u8> from Rust
  picture_type: ArtworkType;
};

export type ArtworkType =
  | 'Other'
  | 'FileIcon'
  | 'OtherFileIcon'
  | 'CoverFront'
  | 'CoverBack'
  | 'LeafletPage'
  | 'Media'
  | 'LeadArtist'
  | 'Artist'
  | 'Conductor'
  | 'Band'
  | 'Composer'
  | 'Lyricist'
  | 'RecordingLocation'
  | 'DuringRecording'
  | 'DuringPerformance'
  | 'MovieScreenCapture'
  | 'BrightColourFish'
  | 'Illustration'
  | 'BandArtistLogotype'
  | 'PublisherStudioLogotype';

export type RecordingFormat = {
  mp3?: { bitrate: number };
  flac?: { compression_level: number };
  wav?: {};
};

export type RecordingConfig = {
  id: string;
  name: string;
  format: RecordingFormat;
  output_directory: string;
  filename_template: string;
  metadata: RecordingMetadata;

  // Advanced options
  auto_stop_on_silence: boolean;
  silence_threshold_db: number;
  silence_duration_sec: number;
  max_duration_minutes?: number;
  max_file_size_mb?: number;
  split_on_interval_minutes?: number;

  // Quality settings
  sample_rate: number;
  channels: number;
  bit_depth: number;
};

export type RecordingSession = {
  id: string;
  config: RecordingConfig;
  start_time: string;
  current_file_path: string;
  temp_file_path?: string;
  duration_seconds: number;
  file_size_bytes: number;
  is_paused: boolean;
  is_recovering: boolean;
  metadata: RecordingMetadata;
  current_levels: [number, number]; // [Left, Right] RMS levels for UI display
};

export type RecordingStatus = {
  is_recording: boolean;
  is_paused: boolean;
  current_session?: RecordingSession; // Fixed to match backend serialization
  active_writers_count: number;
  available_space_gb: number;
  total_recordings: number;
  active_recordings: string[];
};

export type RecordingHistoryEntry = {
  id: string;
  config_name: string;
  file_path: string;
  start_time: string;
  end_time: string;
  duration_seconds: number;
  file_size_bytes: number;
  format: RecordingFormat;
  metadata: RecordingMetadata;
};

export type MetadataPreset = {
  name: string;
  metadata: RecordingMetadata;
};
