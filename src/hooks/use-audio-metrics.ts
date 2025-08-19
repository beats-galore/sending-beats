// Hook for audio metrics - only what metrics display needs
import { useMixerStore } from '../stores';

export const useAudioMetrics = () => {
  // Only select metrics, nothing else
  const metrics = useMixerStore((state) => state.metrics);

  return metrics;
};
