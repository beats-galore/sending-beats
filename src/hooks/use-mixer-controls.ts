// Hook specifically for mixer controls - simplified for always-running mixer
import { useCallback, useMemo } from 'react';

import { useMixerStore } from '../stores';
import { MixerState } from '../types';

export const useMixerControls = () => {
  // Only select the specific state needed for controls
  const hasConfig = useMixerStore((state) => state.config !== null);
  const state = useMixerStore((state) => state.state);

  // Mixer is ready when it has config and is running (always-running mode)
  const isReady = hasConfig && state === MixerState.RUNNING;

  // Only select the specific actions needed
  const addChannel = useMixerStore((state) => state.addChannel);

  // Wrap actions to prevent reference changes
  const handleAddChannel = useCallback(() => {
    void addChannel();
  }, [addChannel]);

  return useMemo(
    () => ({
      isReady,
      onAddChannel: handleAddChannel,
    }),
    [isReady, handleAddChannel]
  );
};
