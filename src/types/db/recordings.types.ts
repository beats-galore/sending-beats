// Recording system types
// Corresponds to src-tauri/src/db/recordings.rs

import type { FileName, FilePath, Timestamp, Uuid } from '../util.types';

export type RecordingConfiguration = {
  id: Uuid<RecordingConfiguration>; // UUID as string
  name: string;
  directory: FilePath;
  format: 'mp3' | 'wave' | 'flac'; // 'mp3', 'wav', 'flac'
  sampleRate: number;
  bitrate?: number; // Nullable for lossless formats like WAV
  filenameTemplate: FileName;
  defaultTitle?: string;
  defaultAlbum?: string;
  defaultGenre?: string;
  defaultArtist?: string;
  defaultArtwork?: string; // Path to default artwork file
  autoStopOnSilence: boolean;
  silenceThresholdDb?: number; // Threshold in dB for silence detection
  maxFileSizeMb?: number; // Maximum file size before splitting
  splitOnIntervalMinutes?: number; // Split recording every N minutes
  channelFormat: string; // 'stereo' or 'mono'
  bitDepth: number; // 16, 24, or 32
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
};

export type Recording = {
  id: Uuid<Recording>; // UUID as string
  recordingConfigId?: Uuid<RecordingConfiguration>; // UUID as string, may be null if config was deleted
  internalDirectory: FilePath; // Internal storage path
  fileName: FileName;
  sizeMb: number;
  format: string; // 'mp3', 'wav', 'flac'
  sampleRate: number;
  bitrate?: number; // Nullable for lossless formats
  durationSeconds: number;
  channelFormat: string; // 'stereo' or 'mono'
  bitDepth: number;

  // Metadata fields
  title?: string;
  album?: string;
  genre?: string;
  artist?: string;
  artwork?: string; // Path to artwork file
  albumArtist?: string;
  composer?: string;
  trackNumber?: number;
  totalTracks?: number;
  discNumber?: number;
  totalDiscs?: number;
  copyright?: string;
  bpm?: number;
  isrc?: string; // International Standard Recording Code
  encoder?: string;
  encodingDate?: string; // ISO timestamp
  comment?: string;
  year?: number;

  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
};

export type RecordingOutput = {
  id: Uuid<RecordingOutput>; // UUID as string
  recordingId: Uuid<Recording>; // UUID as string
  chunkSequence: number; // For splitting large recordings
  outputData: number[]; // Binary audio data as byte array
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
};
