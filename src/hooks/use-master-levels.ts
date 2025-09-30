// Hook for master VU levels - only what master VU meters need
import { useVUMeterStore } from '../stores';

export const useMasterLevels = () => {
  // Only select master levels, nothing else
  const masterLevels = useVUMeterStore((state) => state.masterLevels);

  return masterLevels;
};
