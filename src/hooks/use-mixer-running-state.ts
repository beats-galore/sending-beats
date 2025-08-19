// Hook for mixer running state - only what VU meter polling needs
import { useMixerStore } from '../stores';

export const useMixerRunningState = () => {
  // Only select if mixer is running, nothing else
  const isRunning = useMixerStore((state) => state.state === 'running');

  return isRunning;
};
