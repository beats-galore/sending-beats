// Broadcasting/streaming system types
// Corresponds to src-tauri/src/db/broadcasts.rs

import type { Timestamp, Uuid } from '../util.types';

export type BroadcastConfiguration = {
  id: Uuid<BroadcastConfiguration>; // UUID as string
  name: string;
  serverUrl: string;
  mountPoint: string;
  username: string;
  password: string; // Should be encrypted in practice
  bitrate: number;
  sampleRate: number;
  channelFormat: string; // 'stereo' or 'mono'
  codec: string; // 'mp3', 'aac', 'ogg'
  isVariableBitrate: boolean;
  vbrQuality?: number; // VBR quality 0-9 (if VBR enabled)
  streamName?: string;
  streamDescription?: string;
  streamGenre?: string;
  streamUrl?: string; // Homepage URL for the stream
  shouldAutoReconnect: boolean;
  maxReconnectAttempts: number;
  reconnectDelaySeconds: number;
  connectionTimeoutSeconds: number;
  bufferSizeMs: number;
  enableQualityMonitoring: boolean;
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
  deletedAt?: Timestamp; // ISO timestamp
};

export type Broadcast = {
  id: string; // UUID as string
  broadcastConfigId?: Uuid<BroadcastConfiguration>; // UUID as string, may be null if config was deleted
  sessionName?: string;
  startTime: string; // ISO timestamp
  endTime?: string; // ISO timestamp, null if still active
  durationSeconds?: number; // Total duration (calculated on end)

  // Connection details (snapshot from config at start time)
  serverUrl: string;
  mountPoint: string;
  streamName?: string;

  // Audio format (snapshot from config at start time)
  bitrate: number;
  sampleRate: number;
  channelFormat: string;
  codec: string;
  actualBitrate?: number; // Measured average bitrate

  // Connection statistics
  bytesSent: number;
  packetsSent: number;
  connectionUptimeSeconds: number;
  reconnectCount: number;

  // Quality metrics
  averageBitrateKbps?: number;
  packetLossRate: number;
  latencyMs?: number;
  bufferUnderruns: number;
  encodingErrors: number;

  // Status tracking
  finalStatus?: 'completed' | 'disconnected' | 'error' | 'cancelled';
  lastError?: string; // Last error message if any

  // Listener statistics (if available from server)
  peakListeners?: number;
  totalListenerMinutes?: number;

  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
  deletedAt?: Timestamp; // ISO timestamp
};

export type BroadcastOutput = {
  id: Uuid<BroadcastOutput>; // UUID as string
  broadcastId: string; // UUID as string
  chunkSequence: number; // Sequence number for ordering
  chunkTimestamp: string; // ISO timestamp
  chunkSizeBytes: number;
  encodingDurationMs?: number; // Time taken to encode this chunk
  transmissionDurationMs?: number; // Time taken to transmit this chunk
  audioData?: number[]; // Optional: store actual audio data for analysis
  createdAt: Timestamp; // ISO timestamp
  updatedAt: Timestamp; // ISO timestamp
  deletedAt?: Timestamp; // ISO timestamp
};

// Broadcasting status types for real-time UI
export type BroadcastStatus = {
  isActive: boolean;
  currentSession?: Broadcast;
  connectionHealth: {
    isConnected: boolean;
    latencyMs?: number;
    packetLossRate: number;
    reconnectCount: number;
  };
  streamStats: {
    bytesSent: number;
    averageBitrateKbps?: number;
    uptimeSeconds: number;
    currentListeners?: number;
  };
};
