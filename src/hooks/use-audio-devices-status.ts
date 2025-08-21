// Hook for audio devices status only - loading and error states
import { useMemo } from 'react';

import { useAudioDeviceStore } from '../stores';

export const useAudioDevicesStatus = () => {
  const error = useAudioDeviceStore((state) => state.error);

  return useMemo(
    () => ({
      error,
    }),
    [error]
  );
};
