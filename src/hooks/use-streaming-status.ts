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

export type StreamingServiceStatus = {
  is_running: boolean;
  is_connected: boolean;
  is_streaming: boolean;
  uptime_seconds: number;
  audio_stats: AudioStreamingStats | null;
  icecast_stats: IcecastStreamingStats | null;
  connection_diagnostics: ConnectionDiagnostics;
  bitrate_info: BitrateInfo;
  last_error: string | null;
};

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
      const result = await invoke<StreamingServiceStatus>('get_icecast_streaming_status');
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

  const setVariableBitrate = useCallback(async (enabled: boolean, quality: number) => {
    try {
      await invoke<string>('set_variable_bitrate_streaming', { enabled, quality });
      // Refresh status to get updated VBR info
      await fetchStatus();
    } catch (err) {
      console.error('Failed to set variable bitrate:', err);
      throw err;
    }
  }, [fetchStatus]);

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
