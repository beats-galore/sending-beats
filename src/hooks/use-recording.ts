import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState, useCallback } from 'react';
import type { RecordingConfig, RecordingStatus, RecordingHistoryEntry, RecordingMetadata, MetadataPreset } from '../types/audio.types';

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
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const result = await invoke<RecordingStatus>('get_recording_status');
      setStatus(result);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch recording status:', err);
      setError(err as string);
    } finally {
      setIsLoading(false);
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

  const startRecording = useCallback(async (config: RecordingConfig): Promise<string> => {
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
  }, [fetchStatus]);

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

  const saveConfig = useCallback(async (config: RecordingConfig) => {
    try {
      await invoke<string>('save_recording_config', { config });
      await fetchConfigs(); // Refresh configs
    } catch (err) {
      console.error('Failed to save recording config:', err);
      throw err;
    }
  }, [fetchConfigs]);

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

  const updateSessionMetadata = useCallback(async (metadata: RecordingMetadata): Promise<void> => {
    try {
      await invoke<void>('update_recording_metadata', { metadata });
      await fetchStatus(); // Refresh status to get updated metadata
    } catch (err) {
      console.error('Failed to update session metadata:', err);
      throw err;
    }
  }, [fetchStatus]);

  useEffect(() => {
    // Initial fetch
    fetchStatus();
    fetchConfigs();
    fetchHistory();

    // Set up polling for status updates
    const interval = setInterval(fetchStatus, pollingInterval);

    return () => clearInterval(interval);
  }, [fetchStatus, fetchConfigs, fetchHistory, pollingInterval]);

  const actions: RecordingActions = {
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
  };

  return {
    status,
    configs,
    history,
    isLoading,
    error,
    actions,
  };
};