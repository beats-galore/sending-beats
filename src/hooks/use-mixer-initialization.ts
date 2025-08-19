// Hook specifically for mixer initialization - only what VirtualMixer needs for initialization
import { useMixerStore } from '../stores';

export const useMixerInitialization = () => {
  // Only select initialization-related state
  const hasConfig = useMixerStore((state) => state.config !== null);
  const mixerError = useMixerStore((state) => state.error);
  const initialize = useMixerStore((state) => state.initializeMixer);

  return {
    isReady: hasConfig,
    error: mixerError,
    initialize,
  };
};