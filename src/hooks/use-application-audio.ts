import { useEffect, useMemo } from 'react';

import { useApplicationAudioStore } from '../stores/application-audio-store';

export const useApplicationAudio = () => {
  const store = useApplicationAudioStore();

  useEffect(() => {
    if (store.isLoading || store.initialLoadCompleted) {
      return;
    }
    store.refreshApplications();
  }, [store.refreshApplications]);

  return useMemo(
    () => ({
      availableApps: store.availableApps,
      knownApps: store.knownApps,
      activeCaptures: store.activeCaptures,
      permissionsGranted: store.permissionsGranted,
      isLoading: store.isLoading,
      error: store.error,
      actions: {
        refreshApplications: store.refreshApplications,
        requestPermissions: store.requestPermissions,
        startCapturing: store.startCapturing,
        stopCapturing: store.stopCapturing,
        createMixerInput: store.createMixerInput,
        stopAllCaptures: store.stopAllCaptures,
        clearError: store.clearError,
      },
    }),
    []
  );
};
