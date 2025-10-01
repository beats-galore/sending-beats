import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { ProcessInfo } from '../types/applicationAudio.types';

export type ApplicationAudioState = {
  availableApps: ProcessInfo[];
  knownApps: ProcessInfo[];
  activeCaptures: ProcessInfo[];
  permissionsGranted: boolean;
  isLoading: boolean;
  error: string | null;
};

export const useApplicationAudio = () => {
  const [state, setState] = useState<ApplicationAudioState>({
    availableApps: [],
    knownApps: [],
    activeCaptures: [],
    permissionsGranted: false,
    isLoading: false,
    error: null,
  });

  // Refresh available audio applications
  const refreshApplications = useCallback(async () => {
    console.log('🔄 useApplicationAudio: Starting refreshApplications...');
    setState((prev) => ({ ...prev, isLoading: true, error: null }));

    try {
      console.log('📞 useApplicationAudio: Calling Tauri commands...');
      const [availableApps, knownApps, activeCaptures] = await Promise.all([
        invoke<ProcessInfo[]>('get_available_audio_applications'),
        invoke<ProcessInfo[]>('get_known_audio_applications'),
        invoke<ProcessInfo[]>('get_active_audio_captures'),
      ]);
      console.log('✅ useApplicationAudio: All Tauri commands completed successfully');

      setState((prev) => ({
        ...prev,
        availableApps,
        knownApps,
        activeCaptures,
        permissionsGranted: true,
        isLoading: false,
      }));

      console.log('🎵 Application audio state updated:', {
        availableCount: availableApps.length,
        knownCount: knownApps.length,
        activeCount: activeCaptures.length,
        permissionsGranted: true,
      });
    } catch (error) {
      console.error('❌ useApplicationAudio: Failed to refresh applications:', error);
      console.error('❌ useApplicationAudio: Error details:', JSON.stringify(error, null, 2));
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error: error as string,
      }));
    }
  }, []);

  // Request audio capture permissions
  const requestPermissions = useCallback(async (): Promise<{
    granted: boolean;
    message: string;
  }> => {
    setState((prev) => ({ ...prev, isLoading: true, error: null }));

    try {
      const message = await invoke<string>('request_audio_capture_permissions');
      const granted = message.includes('already granted');

      setState((prev) => ({
        ...prev,
        permissionsGranted: granted,
        isLoading: false,
      }));

      if (granted) {
        console.log('✅ Audio capture permissions granted');
        // Refresh applications now that we have permissions
        await refreshApplications();
      } else {
        console.warn('❌ Audio capture permissions denied or need manual setup');
      }

      return { granted, message };
    } catch (error) {
      console.error('❌ Failed to request permissions:', error);
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error: error as string,
      }));
      return { granted: false, message: 'Failed to check permissions' };
    }
  }, [refreshApplications]);

  // Start capturing from an application
  const startCapturing = useCallback(async (pid: number): Promise<string | null> => {
    setState((prev) => ({ ...prev, isLoading: true, error: null }));

    try {
      const result = await invoke<string>('start_application_audio_capture', { pid });

      // Refresh active captures
      const activeCaptures = await invoke<ProcessInfo[]>('get_active_audio_captures');
      setState((prev) => ({
        ...prev,
        activeCaptures,
        isLoading: false,
      }));

      console.log(`✅ Started capturing from PID ${pid}:`, result);
      return result;
    } catch (error) {
      console.error(`❌ Failed to start capturing from PID ${pid}:`, error);
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error: error as string,
      }));
      return null;
    }
  }, []);

  // Stop capturing from an application
  const stopCapturing = useCallback(async (pid: number): Promise<boolean> => {
    setState((prev) => ({ ...prev, isLoading: true, error: null }));

    try {
      await invoke<string>('stop_application_audio_capture', { pid });

      // Refresh active captures
      const activeCaptures = await invoke<ProcessInfo[]>('get_active_audio_captures');
      setState((prev) => ({
        ...prev,
        activeCaptures,
        isLoading: false,
      }));

      console.log(`✅ Stopped capturing from PID ${pid}`);
      return true;
    } catch (error) {
      console.error(`❌ Failed to stop capturing from PID ${pid}:`, error);
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error: error as string,
      }));
      return false;
    }
  }, []);

  // Create a mixer input for an application
  const createMixerInput = useCallback(async (pid: number): Promise<string | null> => {
    setState((prev) => ({ ...prev, isLoading: true, error: null }));

    try {
      const channelName = await invoke<string>('create_mixer_input_for_application', { pid });

      // Refresh active captures
      const activeCaptures = await invoke<ProcessInfo[]>('get_active_audio_captures');
      setState((prev) => ({
        ...prev,
        activeCaptures,
        isLoading: false,
      }));

      console.log(`✅ Created mixer input for PID ${pid}: ${channelName}`);
      return channelName;
    } catch (error) {
      console.error(`❌ Failed to create mixer input for PID ${pid}:`, error);
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error: error as string,
      }));
      return null;
    }
  }, []);

  // Stop all active captures
  const stopAllCaptures = useCallback(async (): Promise<boolean> => {
    setState((prev) => ({ ...prev, isLoading: true, error: null }));

    try {
      await invoke<string>('stop_all_audio_captures');

      setState((prev) => ({
        ...prev,
        activeCaptures: [],
        isLoading: false,
      }));

      console.log('✅ Stopped all audio captures');
      return true;
    } catch (error) {
      console.error('❌ Failed to stop all captures:', error);
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error: error as string,
      }));
      return false;
    }
  }, []);

  // Clear error state
  const clearError = useCallback(() => {
    setState((prev) => ({ ...prev, error: null }));
  }, []);

  // Initial load
  useEffect(() => {
    refreshApplications();
  }, [refreshApplications]);

  return {
    ...state,
    actions: {
      refreshApplications,
      requestPermissions,
      startCapturing,
      stopCapturing,
      createMixerInput,
      stopAllCaptures,
      clearError,
    },
  };
};
