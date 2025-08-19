// Custom hook for mixer state management
import { useCallback, useMemo } from 'react';

import { useMixerStore } from '../stores';
import { MixerState } from '../types';

export const useMixerState = () => {
  const {
    config,
    state,
    error,
    metrics,
    masterLevels,
    initializeMixer,
    startMixer,
    stopMixer,
    addChannel,
    updateChannel,
    updateMasterGain,
    setError,
    clearError,
  } = useMixerStore();

  // Derived state
  const isRunning = state === MixerState.RUNNING;
  const isStarting = state === MixerState.STARTING;
  const isStopping = state === MixerState.STOPPING;
  const isStopped = state === MixerState.STOPPED;
  const hasError = state === MixerState.ERROR;
  const isReady = config !== null && isStopped;

  // Channel management helpers
  const getChannelById = useCallback(
    (channelId: number) => {
      return config?.channels.find((channel) => channel.id === channelId) || null;
    },
    [config?.channels]
  );

  const updateChannelGain = useCallback(
    async (channelId: number, gain: number) => {
      await updateChannel(channelId, { gain });
    },
    [updateChannel]
  );

  const updateChannelPan = useCallback(
    async (channelId: number, pan: number) => {
      await updateChannel(channelId, { pan });
    },
    [updateChannel]
  );

  const toggleChannelMute = useCallback(
    async (channelId: number) => {
      const channel = getChannelById(channelId);
      if (channel) {
        await updateChannel(channelId, { muted: !channel.muted });
      }
    },
    [getChannelById, updateChannel]
  );

  const toggleChannelSolo = useCallback(
    async (channelId: number) => {
      const channel = getChannelById(channelId);
      if (channel) {
        await updateChannel(channelId, { solo: !channel.solo });
      }
    },
    [getChannelById, updateChannel]
  );

  const setChannelInputDevice = useCallback(
    async (channelId: number, deviceId: string) => {
      await updateChannel(channelId, { input_device_id: deviceId });
    },
    [updateChannel]
  );

  // EQ controls
  const updateChannelEQ = useCallback(
    async (
      channelId: number,
      eq: {
        eq_low_gain?: number;
        eq_mid_gain?: number;
        eq_high_gain?: number;
      }
    ) => {
      await updateChannel(channelId, eq);
    },
    [updateChannel]
  );

  // Compressor controls
  const updateChannelCompressor = useCallback(
    async (
      channelId: number,
      comp: {
        comp_threshold?: number;
        comp_ratio?: number;
        comp_attack?: number;
        comp_release?: number;
        comp_enabled?: boolean;
      }
    ) => {
      await updateChannel(channelId, comp);
    },
    [updateChannel]
  );

  // Limiter controls
  const updateChannelLimiter = useCallback(
    async (
      channelId: number,
      limiter: {
        limiter_threshold?: number;
        limiter_enabled?: boolean;
      }
    ) => {
      await updateChannel(channelId, limiter);
    },
    [updateChannel]
  );

  // Master controls
  const setMasterGain = useCallback(
    async (gain: number) => {
      await updateMasterGain(gain);
    },
    [updateMasterGain]
  );

  // Initialize mixer with error handling
  const initialize = useCallback(async () => {
    try {
      clearError();
      await initializeMixer();
      return true;
    } catch (err) {
      console.error('Failed to initialize mixer:', err);
      return false;
    }
  }, [initializeMixer, clearError]);

  // Start mixer with error handling
  const start = useCallback(async () => {
    try {
      clearError();
      await startMixer();
      return true;
    } catch (err) {
      console.error('Failed to start mixer:', err);
      return false;
    }
  }, [startMixer, clearError]);

  // Stop mixer with error handling
  const stop = useCallback(async () => {
    try {
      clearError();
      await stopMixer();
      return true;
    } catch (err) {
      console.error('Failed to stop mixer:', err);
      return false;
    }
  }, [stopMixer, clearError]);

  // Add channel with error handling
  const createChannel = useCallback(async () => {
    try {
      clearError();
      await addChannel();
      return true;
    } catch (err) {
      console.error('Failed to add channel:', err);
      return false;
    }
  }, [addChannel, clearError]);

  return useMemo(
    () => ({
      // State
      config,
      state,
      error,
      metrics,
      masterLevels,

      // Derived state
      isRunning,
      isStarting,
      isStopping,
      isStopped,
      hasError,
      isReady,

      // Core actions
      initialize,
      start,
      stop,
      createChannel,

      // Channel helpers
      getChannelById,
      updateChannelGain,
      updateChannelPan,
      toggleChannelMute,
      toggleChannelSolo,
      setChannelInputDevice,

      // Effects
      updateChannelEQ,
      updateChannelCompressor,
      updateChannelLimiter,

      // Master controls
      setMasterGain,

      // Error handling
      clearError,
      setError,
    }),
    [
      config,
      state,
      error,
      metrics,
      masterLevels,
      isRunning,
      isStarting,
      isStopping,
      isStopped,
      hasError,
      isReady,
      initialize,
      start,
      stop,
      createChannel,
      getChannelById,
      updateChannelGain,
      updateChannelPan,
      toggleChannelMute,
      toggleChannelSolo,
      setChannelInputDevice,
      updateChannelEQ,
      updateChannelCompressor,
      updateChannelLimiter,
      setMasterGain,
      clearError,
      setError,
    ]
  );
};
