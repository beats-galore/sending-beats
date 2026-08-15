import { invoke } from '@tauri-apps/api/core';
import { useState, useCallback } from 'react';

export type StreamingControlsState = {
  isStarting: boolean;
  isStopping: boolean;
  error: string | null;
};

export type StreamingControlsActions = {
  startStreaming: (castConfigurationId: string) => Promise<void>;
  stopStreaming: () => Promise<void>;
  updateMetadata: (title: string, artist: string) => Promise<void>;
  clearError: () => void;
};

export const useStreamingControls = () => {
  const [state, setState] = useState<StreamingControlsState>({
    isStarting: false,
    isStopping: false,
    error: null,
  });

  const updateState = useCallback((updates: Partial<StreamingControlsState>) => {
    setState((prev) => ({ ...prev, ...updates }));
  }, []);

  const startStreaming = useCallback(async (castConfigurationId: string) => {
    updateState({ isStarting: true, error: null });

    try {
      // The station is named rather than its fields being sent: the password is
      // in the keychain, and the backend registers the stream under this id so
      // the broadcast can be routed to.
      await invoke<string>('start_cast', { id: castConfigurationId });
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
