import { invoke } from '@tauri-apps/api/core';
import { useState, useCallback } from 'react';

export type StreamConfig = {
  server_host: string;
  server_port: number;
  mount_point: string;
  password: string;
  stream_name: string;
  bitrate: number;
};

export type StreamingControlsState = {
  isConnecting: boolean;
  isStarting: boolean;
  isStopping: boolean;
  error: string | null;
};

export type StreamingControlsActions = {
  initialize: (config: StreamConfig) => Promise<void>;
  startStreaming: () => Promise<void>;
  stopStreaming: () => Promise<void>;
  updateMetadata: (title: string, artist: string) => Promise<void>;
  clearError: () => void;
};

export const useStreamingControls = () => {
  const [state, setState] = useState<StreamingControlsState>({
    isConnecting: false,
    isStarting: false,
    isStopping: false,
    error: null,
  });

  const updateState = useCallback((updates: Partial<StreamingControlsState>) => {
    setState((prev) => ({ ...prev, ...updates }));
  }, []);

  const initialize = useCallback(
    async (config: StreamConfig) => {
      updateState({ isConnecting: true, error: null });

      try {
        await invoke<string>('initialize_icecast_streaming', {
          serverHost: config.server_host,
          serverPort: config.server_port,
          mountPoint: config.mount_point,
          password: config.password,
          streamName: config.stream_name,
          bitrate: config.bitrate,
        });

        updateState({ isConnecting: false });
      } catch (err) {
        console.error('Failed to initialize streaming:', err);
        updateState({
          isConnecting: false,
          error: `Failed to initialize streaming: ${err}`,
        });
        throw err;
      }
    },
    [updateState]
  );

  const startStreaming = useCallback(async () => {
    updateState({ isStarting: true, error: null });

    try {
      await invoke<string>('start_icecast_streaming');
      updateState({ isStarting: false });
    } catch (err) {
      console.error('Failed to start streaming:', err);
      updateState({
        isStarting: false,
        error: `Failed to start streaming: ${err}`,
      });
      throw err;
    }
  }, [updateState]);

  const stopStreaming = useCallback(async () => {
    updateState({ isStopping: true, error: null });

    try {
      await invoke<string>('stop_icecast_streaming');
      updateState({ isStopping: false });
    } catch (err) {
      console.error('Failed to stop streaming:', err);
      updateState({
        isStopping: false,
        error: `Failed to stop streaming: ${err}`,
      });
      throw err;
    }
  }, [updateState]);

  const updateMetadata = useCallback(
    async (title: string, artist: string) => {
      try {
        await invoke<string>('update_icecast_metadata', { title, artist });
      } catch (err) {
        console.error('Failed to update metadata:', err);
        updateState({
          error: `Failed to update metadata: ${err}`,
        });
        throw err;
      }
    },
    [updateState]
  );

  const clearError = useCallback(() => {
    updateState({ error: null });
  }, [updateState]);

  const actions: StreamingControlsActions = {
    initialize,
    startStreaming,
    stopStreaming,
    updateMetadata,
    clearError,
  };

  return {
    state,
    actions,
  };
};
