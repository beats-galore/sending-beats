import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState, useCallback } from 'react';

export type ConnectionDiagnostics = {
  latency_ms: number | null;
  packet_loss_rate: number;
  connection_stability: number;
  reconnect_attempts: number;
  time_since_last_reconnect_seconds: number | null;
  connection_uptime_seconds: number | null;
};

export type BitrateInfo = {
  current_bitrate: number;
  available_bitrates: number[];
  codec: string;
  is_variable_bitrate: boolean;
  vbr_quality: number;
  actual_bitrate: number | null;
};

export type AudioStreamingStats = {
  samples_processed: number;
  samples_per_second: number;
  buffer_overruns: number;
  encoding_errors: number;
};

export type IcecastStreamingStats = {
  bytes_sent: number;
  packets_sent: number;
  connection_duration_seconds: number;
  average_bitrate_kbps: number;
};

/**
 * What an Impulse broadcast has done, and what the far end says about it.
 *
 * `on_air` is the far end's own answer to the last segment rather than anything
 * measured here. It is the stronger claim: a connection can be open to a server
 * that is putting nothing in front of listeners, but an acknowledged segment was
 * genuinely delivered.
 */
export type ImpulseStreamingStats = {
  segments_sent: number;
  bytes_sent: number;
  /** Cut because the uploader could not keep up, each one a gap in the audio */
  segments_dropped: number;
  send_errors: number;
  on_air: boolean;
  media_sequence: number;
};

export type StreamingServiceStatus = {
  is_running: boolean;
  is_connected: boolean;
  is_streaming: boolean;
  uptime_seconds: number;
  audio_stats: AudioStreamingStats | null;
  icecast_stats: IcecastStreamingStats | null;
  /** Present only while Impulse is the transmitter on air */
  impulse_stats: ImpulseStreamingStats | null;
  connection_diagnostics: ConnectionDiagnostics;
  bitrate_info: BitrateInfo;
  last_error: string | null;
};

/** Bytes put on the wire, whichever transmitter put them there. */
export const bytesSent = (status: StreamingServiceStatus | null): number | null =>
  status?.impulse_stats?.bytes_sent ?? status?.icecast_stats?.bytes_sent ?? null;

export type StreamingActions = {
  refreshStatus: () => Promise<void>;
  setBitrate: (bitrate: number) => Promise<void>;
  getAvailableBitrates: () => Promise<number[]>;
  getCurrentBitrate: () => Promise<number>;
  setVariableBitrate: (enabled: boolean, quality: number) => Promise<void>;
  getVariableBitrateSettings: () => Promise<[boolean, number]>;
};

export const useStreamingStatus = (pollingInterval = 2000) => {
  const [status, setStatus] = useState<StreamingServiceStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      // Asked of the cast layer rather than of Icecast: the backend knows which
      // transmitter is on air, and asking one of them directly reports a station
      // that is off while missing the one that is on.
      const result = await invoke<StreamingServiceStatus>('get_cast_status');
      setStatus(result);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch streaming status:', err);
      setError(err as string);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    setIsLoading(true);
    await fetchStatus();
  }, [fetchStatus]);

  const setBitrate = useCallback(
    async (bitrate: number) => {
      try {
        await invoke<string>('set_stream_bitrate', { bitrate });
        // Refresh status to get updated bitrate info
        await fetchStatus();
      } catch (err) {
        console.error('Failed to set bitrate:', err);
        throw err;
      }
    },
    [fetchStatus]
  );

  const getAvailableBitrates = useCallback(async (): Promise<number[]> => {
    try {
      return await invoke<number[]>('get_available_stream_bitrates');
    } catch (err) {
      console.error('Failed to get available bitrates:', err);
      throw err;
    }
  }, []);

  const getCurrentBitrate = useCallback(async (): Promise<number> => {
    try {
      return await invoke<number>('get_current_stream_bitrate');
    } catch (err) {
      console.error('Failed to get current bitrate:', err);
      throw err;
    }
  }, []);

  const setVariableBitrate = useCallback(
    async (enabled: boolean, quality: number) => {
      try {
        await invoke<string>('set_variable_bitrate_streaming', { enabled, quality });
        // Refresh status to get updated VBR info
        await fetchStatus();
      } catch (err) {
        console.error('Failed to set variable bitrate:', err);
        throw err;
      }
    },
    [fetchStatus]
  );

  const getVariableBitrateSettings = useCallback(async (): Promise<[boolean, number]> => {
    try {
      return await invoke<[boolean, number]>('get_variable_bitrate_settings');
    } catch (err) {
      console.error('Failed to get variable bitrate settings:', err);
      throw err;
    }
  }, []);

  useEffect(() => {
    fetchStatus();

    const interval = setInterval(fetchStatus, pollingInterval);

    return () => clearInterval(interval);
  }, [fetchStatus, pollingInterval]);

  const actions: StreamingActions = {
    refreshStatus,
    setBitrate,
    getAvailableBitrates,
    getCurrentBitrate,
    setVariableBitrate,
    getVariableBitrateSettings,
  };

  return {
    status,
    isLoading,
    error,
    actions,
  };
};
