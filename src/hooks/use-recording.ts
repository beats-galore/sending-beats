import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState, useCallback, useMemo } from 'react';

import type {
  RecordingConfig,
  RecordingStatus,
  RecordingHistoryEntry,
  RecordingMetadata,
  MetadataPreset,
  RecordingFormat,
} from '../types/audio.types';

// Re-export types for convenience
export type {
  RecordingConfig,
  RecordingStatus,
  RecordingHistoryEntry,
  RecordingMetadata,
  MetadataPreset,
  RecordingFormat,
};

export type RecordingActions = {
  startRecording: (config: RecordingConfig) => Promise<string>;
  stopRecording: () => Promise<RecordingHistoryEntry | null>;
  getStatus: () => Promise<void>;
  saveConfig: (config: RecordingConfig) => Promise<void>;
  getConfigs: () => Promise<RecordingConfig[]>;
  getHistory: () => Promise<RecordingHistoryEntry[]>;
  createDefaultConfig: () => Promise<RecordingConfig>;
  getMetadataPresets: () => Promise<MetadataPreset[]>;
  getRecordingPresets: () => Promise<RecordingConfig[]>;
  updateSessionMetadata: (metadata: RecordingMetadata) => Promise<void>;
};

export const useRecording = (pollingInterval = 1000) => {
  const [status, setStatus] = useState<RecordingStatus | null>(null);
  const [configs, setConfigs] = useState<RecordingConfig[]>([]);
  const [history, setHistory] = useState<RecordingHistoryEntry[]>([]);

  const [error, setError] = useState<string | null>(null);

  // Status carries free disk space and the take count, which the transport shows
  // while idle, and the backend can stop a recording on its own once a silence,
  // duration or size limit is hit. Both need polling that does not depend on
  // what the frontend believes is happening.
  const fetchStatus = useCallback(async () => {
    try {
      const result = await invoke<RecordingStatus>('get_recording_status');
      setStatus(result);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch recording status:', err);
      setError(err as string);
    }
  }, []);

  const fetchConfigs = useCallback(async () => {
    try {
      const result = await invoke<RecordingConfig[]>('get_recording_configs');
      setConfigs(result);
    } catch (err) {
      console.error('Failed to fetch recording configs:', err);
    }
  }, []);

  const fetchHistory = useCallback(async () => {
    try {
      const result = await invoke<RecordingHistoryEntry[]>('get_recording_history');
      setHistory(result);
    } catch (err) {
      console.error('Failed to fetch recording history:', err);
    }
  }, []);

  const startRecording = useCallback(
    async (config: RecordingConfig): Promise<string> => {
      try {
        console.log('useRecording: Calling start_recording with config:', config);
        const sessionId = await invoke<string>('start_recording', { config });
        console.log('useRecording: Got session ID:', sessionId);
        await fetchStatus(); // Refresh status immediately
        return sessionId;
      } catch (err) {
        console.error('useRecording: Failed to start recording:', err);
        throw err;
      }
    },
    [fetchStatus]
  );

  const stopRecording = useCallback(async (): Promise<RecordingHistoryEntry | null> => {
    try {
      const historyEntry = await invoke<RecordingHistoryEntry | null>('stop_recording');
      await Promise.all([fetchStatus(), fetchHistory()]); // Refresh both status and history
      return historyEntry;
    } catch (err) {
      console.error('Failed to stop recording:', err);
      throw err;
    }
  }, [fetchStatus, fetchHistory]);

  const saveConfig = useCallback(
    async (config: RecordingConfig) => {
      try {
        await invoke<string>('save_recording_config', { config });
        await fetchConfigs(); // Refresh configs
      } catch (err) {
        console.error('Failed to save recording config:', err);
        throw err;
      }
    },
    [fetchConfigs]
  );

  const createDefaultConfig = useCallback(async (): Promise<RecordingConfig> => {
    try {
      return await invoke<RecordingConfig>('create_default_recording_config');
    } catch (err) {
      console.error('Failed to create default config:', err);
      throw err;
    }
  }, []);

  const getStatus = useCallback(async () => {
    await fetchStatus();
  }, [fetchStatus]);

  const getConfigs = useCallback(async (): Promise<RecordingConfig[]> => {
    await fetchConfigs();
    return configs;
  }, [fetchConfigs, configs]);

  const getHistory = useCallback(async (): Promise<RecordingHistoryEntry[]> => {
    await fetchHistory();
    return history;
  }, [fetchHistory, history]);

  const getMetadataPresets = useCallback(async (): Promise<MetadataPreset[]> => {
    try {
      return await invoke<MetadataPreset[]>('get_metadata_presets');
    } catch (err) {
      console.error('Failed to get metadata presets:', err);
      return [];
    }
  }, []);

  const getRecordingPresets = useCallback(async (): Promise<RecordingConfig[]> => {
    try {
      return await invoke<RecordingConfig[]>('get_recording_presets');
    } catch (err) {
      console.error('Failed to get recording presets:', err);
      return [];
    }
  }, []);

  const updateSessionMetadata = useCallback(
    async (metadata: RecordingMetadata): Promise<void> => {
      try {
        await invoke<void>('update_recording_metadata', { metadata });
        await fetchStatus(); // Refresh status to get updated metadata
      } catch (err) {
        console.error('Failed to update session metadata:', err);
        throw err;
      }
    },
    [fetchStatus]
  );

  useEffect(() => {
    // Initial fetch
    void fetchStatus();
    void fetchConfigs();
    void fetchHistory();

    // Set up polling for status updates
    const interval = setInterval(() => void fetchStatus(), pollingInterval);

    return () => clearInterval(interval);
  }, [fetchStatus, fetchConfigs, fetchHistory, pollingInterval]);

  const actions: RecordingActions = useMemo(
    () => ({
      startRecording,
      stopRecording,
      getStatus,
      saveConfig,
      getConfigs,
      getHistory,
      createDefaultConfig,
      getMetadataPresets,
      getRecordingPresets,
      updateSessionMetadata,
    }),
    [
      startRecording,
      stopRecording,
      getStatus,
      saveConfig,
      getConfigs,
      getHistory,
      createDefaultConfig,
      getMetadataPresets,
      getRecordingPresets,
      updateSessionMetadata,
    ]
  );

  return useMemo(
    () => ({
      status,
      configs,
      history,

      error,
      actions,
    }),
    [status, configs, history, error, actions]
  );
};
