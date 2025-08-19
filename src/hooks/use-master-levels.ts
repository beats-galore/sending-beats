// Hook for master VU levels - only what master VU meters need
import { useMixerStore } from '../stores';

export const useMasterLevels = () => {
  // Only select master levels, nothing else
  const masterLevels = useMixerStore((state) => state.masterLevels);

  return masterLevels;
};
