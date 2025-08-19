// Hook specifically for mixer controls - only reads what MixerControls component needs
import { useCallback } from 'react';
import { useMixerStore } from '../stores';

export const useMixerControls = () => {
  // Only select the specific state needed for controls
  const isReady = useMixerStore((state) => {
    return state.config !== null && state.state === 'stopped';
  });
  
  const isRunning = useMixerStore((state) => state.state === 'running');
  
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

  return {
    isReady,
    isRunning,
    onStart: handleStart,
    onStop: handleStop,
    onAddChannel: handleAddChannel,
  };
};