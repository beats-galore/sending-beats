import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import { useApplicationAudio } from './useApplicationAudio';

export const useStartupPermissionCheck = () => {
  const [showPermissionModal, setShowPermissionModal] = useState(false);
  const [hasCheckedOnStartup, setHasCheckedOnStartup] = useState(false);
  const applicationAudio = useApplicationAudio();

  // Check permissions on startup
  useEffect(() => {
    if (!hasCheckedOnStartup && applicationAudio.knownApps.length > 0) {
      setHasCheckedOnStartup(true);

      // If there are known apps but no permissions, show the modal
      // if (!applicationAudio.permissionsGranted) {
      //   console.log('🔐 Startup permission check: Known apps found but permissions not granted');
      //   setShowPermissionModal(true);
      // } else {
      //   console.log('✅ Startup permission check: Permissions already granted');
      // }
    }
  }, [hasCheckedOnStartup, applicationAudio.knownApps.length, applicationAudio.permissionsGranted]);

  const handleOpenSystemPreferences = useCallback(async () => {
    try {
      // Try to open System Preferences directly to Privacy settings
      await invoke('open_system_preferences_privacy');
    } catch (error) {
      console.warn('Could not open System Preferences directly, user will need to open manually');
    }
  }, []);

  const handleCloseModal = useCallback(() => {
    setShowPermissionModal(false);
  }, []);

  const handleRequestPermissions = useCallback(async () => {
    const result = await applicationAudio.actions.requestPermissions();

    if (result.granted) {
      setShowPermissionModal(false);
    } else {
      // If permissions still not granted, keep modal open
      console.log('Permissions still not granted after request');
    }

    return result;
  }, [applicationAudio.actions]);

  return {
    showPermissionModal,
    handleCloseModal,
    handleOpenSystemPreferences,
    handleRequestPermissions,
    isLoading: applicationAudio.isLoading,
    permissionsGranted: applicationAudio.permissionsGranted,
  };
};
