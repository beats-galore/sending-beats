import { invoke } from '@tauri-apps/api/core';
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

import type { AvailableApplicationInfo, ProcessInfo } from '../types/applicationAudio.types';

type ApplicationAudioStore = {
  availableApps: ProcessInfo[];
  knownApps: ProcessInfo[];
  activeCaptures: ProcessInfo[];
  allApplications: AvailableApplicationInfo[];
  permissionsGranted: boolean;
  isLoading: boolean;
  error: string | null;
  initialLoadCompleted: boolean;

  refreshApplications: () => Promise<void>;
  loadAllApplications: () => Promise<void>;
  addApplication: (
    bundleIdentifier: string,
    applicationName: string,
    operatingSystem: string
  ) => Promise<string>;
  updateApplicationName: (bundleIdentifier: string, newApplicationName: string) => Promise<string>;
  removeApplication: (bundleIdentifier: string) => Promise<string>;
  requestPermissions: () => Promise<{ granted: boolean; message: string }>;
  startCapturing: (pid: number) => Promise<string | null>;
  stopCapturing: (pid: number) => Promise<boolean>;
  clearError: () => void;
};

export const useApplicationAudioStore = create<ApplicationAudioStore>()(
  subscribeWithSelector((set, get) => ({
    availableApps: [],
    knownApps: [],
    activeCaptures: [],
    allApplications: [],
    permissionsGranted: false,
    isLoading: false,
    error: null,
    initialLoadCompleted: false,

    loadAllApplications: async () => {
      set({ isLoading: true, error: null });
      try {
        const apps = await invoke<AvailableApplicationInfo[]>('get_all_available_applications');
        set({ allApplications: apps, isLoading: false });
      } catch (error) {
        console.error('Failed to load all applications:', error);
        set({ isLoading: false, error: error as string });
      }
    },

    addApplication: async (
      bundleIdentifier: string,
      applicationName: string,
      operatingSystem: string
    ) => {
      set({ error: null });
      try {
        const result = await invoke<string>('add_audio_application', {
          bundleIdentifier,
          applicationName,
          operatingSystem,
        });
        await get().loadAllApplications();
        return result;
      } catch (error) {
        console.error('Failed to add application:', error);
        set({ error: error as string });
        throw error;
      }
    },

    updateApplicationName: async (bundleIdentifier: string, newApplicationName: string) => {
      set({ error: null });
      try {
        const result = await invoke<string>('update_audio_application_name', {
          bundleIdentifier,
          newApplicationName,
        });
        await get().loadAllApplications();
        return result;
      } catch (error) {
        console.error('Failed to update application name:', error);
        set({ error: error as string });
        throw error;
      }
    },

    removeApplication: async (bundleIdentifier: string) => {
      set({ error: null });
      try {
        const result = await invoke<string>('remove_audio_application', {
          bundleIdentifier,
        });
        await get().loadAllApplications();
        return result;
      } catch (error) {
        console.error('Failed to remove application:', error);
        set({ error: error as string });
        throw error;
      }
    },

    refreshApplications: async () => {
      console.log('🔄 useApplicationAudioStore: Starting refreshApplications...');
      set({ isLoading: true, error: null });

      try {
        console.log('📞 useApplicationAudioStore: Calling Tauri commands...');
        const [knownApps] = await Promise.all([
          invoke<ProcessInfo[]>('get_known_audio_applications'),
        ]);
        console.log('✅ useApplicationAudioStore: All Tauri commands completed successfully');

        set({
          knownApps,
          activeCaptures: [],
          permissionsGranted: true,
          isLoading: false,
          initialLoadCompleted: true,
        });

        console.log('🎵 Application audio state updated:', {
          knownCount: knownApps,

          permissionsGranted: true,
        });
      } catch (error) {
        console.error('❌ useApplicationAudioStore: Failed to refresh applications:', error);
        console.error(
          '❌ useApplicationAudioStore: Error details:',
          JSON.stringify(error, null, 2)
        );
        set({
          isLoading: false,
          initialLoadCompleted: true,
          error: error as string,
        });
      }
    },

    requestPermissions: async () => {
      set({ isLoading: true, error: null });

      try {
        const message = await invoke<string>('request_audio_capture_permissions');
        const granted = message.includes('already granted');

        set({
          permissionsGranted: granted,
          isLoading: false,
        });

        if (granted) {
          console.log('✅ Audio capture permissions granted');
          await get().refreshApplications();
        } else {
          console.warn('❌ Audio capture permissions denied or need manual setup');
        }

        return { granted, message };
      } catch (error) {
        console.error('❌ Failed to request permissions:', error);
        set({
          isLoading: false,
          error: error as string,
        });
        return { granted: false, message: 'Failed to check permissions' };
      }
    },

    startCapturing: async (pid: number) => {
      set({ isLoading: true, error: null });

      try {
        const result = await invoke<string>('start_application_audio_capture', { pid });

        const activeCaptures = await invoke<ProcessInfo[]>('get_active_audio_captures');
        set({
          activeCaptures,
          isLoading: false,
        });

        console.log(`✅ Started capturing from PID ${pid}:`, result);
        return result;
      } catch (error) {
        console.error(`❌ Failed to start capturing from PID ${pid}:`, error);
        set({
          isLoading: false,
          error: error as string,
        });
        return null;
      }
    },

    stopCapturing: async (pid: number) => {
      set({ isLoading: true, error: null });

      try {
        await invoke<string>('stop_application_audio_capture', { pid });

        const activeCaptures = await invoke<ProcessInfo[]>('get_active_audio_captures');
        set({
          activeCaptures,
          isLoading: false,
        });

        console.log(`✅ Stopped capturing from PID ${pid}`);
        return true;
      } catch (error) {
        console.error(`❌ Failed to stop capturing from PID ${pid}:`, error);
        set({
          isLoading: false,
          error: error as string,
        });
        return false;
      }
    },

    clearError: () => {
      set({ error: null });
    },
  }))
);
