import { useEffect, useMemo } from 'react';

import { useApplicationAudioStore } from '../stores/application-audio-store';

export const useApplicationAudio = () => {
  const availableApps = useApplicationAudioStore((state) => state.availableApps);
  const knownApps = useApplicationAudioStore((state) => state.knownApps);
  const activeCaptures = useApplicationAudioStore((state) => state.activeCaptures);
  const permissionsGranted = useApplicationAudioStore((state) => state.permissionsGranted);
  const isLoading = useApplicationAudioStore((state) => state.isLoading);
  const initialLoadCompleted = useApplicationAudioStore((state) => state.initialLoadCompleted);
  const error = useApplicationAudioStore((state) => state.error);
  const refreshApplications = useApplicationAudioStore((state) => state.refreshApplications);
  const requestPermissions = useApplicationAudioStore((state) => state.requestPermissions);
  const startCapturing = useApplicationAudioStore((state) => state.startCapturing);
  const stopCapturing = useApplicationAudioStore((state) => state.stopCapturing);
  const clearError = useApplicationAudioStore((state) => state.clearError);

  useEffect(() => {
    if (isLoading || initialLoadCompleted) {
      return;
    }
    refreshApplications();
  }, [isLoading, initialLoadCompleted, refreshApplications]);

  return useMemo(
    () => ({
      availableApps,
      knownApps,
      activeCaptures,
      permissionsGranted,
      isLoading,
      error,
      actions: {
        refreshApplications,
        requestPermissions,
        startCapturing,
        stopCapturing,
        clearError,
      },
    }),
    [
      availableApps,
      knownApps,
      activeCaptures,
      permissionsGranted,
      isLoading,
      error,
      refreshApplications,
      requestPermissions,
      startCapturing,
      stopCapturing,
      clearError,
    ]
  );
};
