// Hook specifically for mixer controls - only reads what MixerControls component needs
import { useCallback, useMemo } from 'react';
import { useMixerStore } from '../stores';

export const useMixerControls = () => {
  // Only select the specific state needed for controls
  const hasConfig = useMixerStore((state) => state.config !== null);
  const state = useMixerStore((state) => state.state);

  const isReady = hasConfig && state === 'stopped';
  const isRunning = state === 'running';
  const canStop = hasConfig && isRunning;

  // Only select the specific actions needed
  const start = useMixerStore((state) => state.startMixer);
  const stop = useMixerStore((state) => state.stopMixer);
  const addChannel = useMixerStore((state) => state.addChannel);

  // Wrap actions to prevent reference changes
  const handleStart = useCallback(() => {
    void start();
  }, [start]);

  const handleStop = useCallback(() => {
    void stop();
  }, [stop]);

  const handleAddChannel = useCallback(() => {
    void addChannel();
  }, [addChannel]);

  return useMemo(
    () => ({
      isReady,
      isRunning,
      canStop,
      onStart: handleStart,
      onStop: handleStop,
      onAddChannel: handleAddChannel,
    }),
    [isReady, isRunning, canStop, handleStart, handleStop, handleAddChannel]
  );
};
