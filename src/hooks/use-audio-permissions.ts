// Custom hook for managing audio capture permissions
import { useState, useEffect, useCallback } from 'react';

import { mixerService } from '../services';

export type AudioPermissionState = {
  hasPermission: boolean | null; // null = checking, true = granted, false = denied
  isLoading: boolean;
  error: string | null;
  permissionInstructions: string | null;
}

export const useAudioPermissions = () => {
  const [permissionState, setPermissionState] = useState<AudioPermissionState>({
    hasPermission: null,
    isLoading: false,
    error: null,
    permissionInstructions: null,
  });

  const checkPermissions = useCallback(async () => {
    setPermissionState(prev => ({ ...prev, isLoading: true, error: null }));
    
    try {
      const hasPermission = await mixerService.checkAudioCapturePermissions();
      setPermissionState({
        hasPermission,
        isLoading: false,
        error: null,
        permissionInstructions: null,
      });
      return hasPermission;
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      setPermissionState({
        hasPermission: false,
        isLoading: false,
        error: errorMessage,
        permissionInstructions: null,
      });
      return false;
    }
  }, []);

  const requestPermissions = useCallback(async () => {
    setPermissionState(prev => ({ ...prev, isLoading: true, error: null }));
    
    try {
      const instructions = await mixerService.requestAudioCapturePermissions();
      const hasPermission = await mixerService.checkAudioCapturePermissions();
      
      setPermissionState({
        hasPermission,
        isLoading: false,
        error: null,
        permissionInstructions: instructions,
      });
      
      return { hasPermission, instructions };
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      setPermissionState({
        hasPermission: false,
        isLoading: false,
        error: errorMessage,
        permissionInstructions: null,
      });
      return { hasPermission: false, instructions: null };
    }
  }, []);

  // Check permissions on mount
  useEffect(() => {
    void checkPermissions();
  }, [checkPermissions]);

  return {
    ...permissionState,
    checkPermissions,
    requestPermissions,
    refreshPermissions: checkPermissions, // Alias for convenience
  };
};