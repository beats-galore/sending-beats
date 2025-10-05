import { invoke } from '@tauri-apps/api/core';
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

import type { ProcessInfo } from '../types/applicationAudio.types';

type ApplicationAudioStore = {
  availableApps: ProcessInfo[];
  knownApps: ProcessInfo[];
  activeCaptures: ProcessInfo[];
  permissionsGranted: boolean;
  isLoading: boolean;
  error: string | null;
  initialLoadCompleted: boolean;

  refreshApplications: () => Promise<void>;
  requestPermissions: () => Promise<{ granted: boolean; message: string }>;
  startCapturing: (pid: number) => Promise<string | null>;
  stopCapturing: (pid: number) => Promise<boolean>;
  createMixerInput: (pid: number) => Promise<string | null>;
  stopAllCaptures: () => Promise<boolean>;
  clearError: () => void;
};

export const useApplicationAudioStore = create<ApplicationAudioStore>()(
  subscribeWithSelector((set, get) => ({
    availableApps: [],
    knownApps: [],
    activeCaptures: [],
    permissionsGranted: false,
    isLoading: false,
    error: null,
    initialLoadCompleted: false,

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

    createMixerInput: async (pid: number) => {
      set({ isLoading: true, error: null });

      try {
        const channelName = await invoke<string>('create_mixer_input_for_application', { pid });

        const activeCaptures = await invoke<ProcessInfo[]>('get_active_audio_captures');
        set({
          activeCaptures,
          isLoading: false,
        });

        console.log(`✅ Created mixer input for PID ${pid}: ${channelName}`);
        return channelName;
      } catch (error) {
        console.error(`❌ Failed to create mixer input for PID ${pid}:`, error);
        set({
          isLoading: false,
          error: error as string,
        });
        return null;
      }
    },

    stopAllCaptures: async () => {
      set({ isLoading: true, error: null });

      try {
        await invoke<string>('stop_all_audio_captures');

        set({
          activeCaptures: [],
          isLoading: false,
        });

        console.log('✅ Stopped all audio captures');
        return true;
      } catch (error) {
        console.error('❌ Failed to stop all captures:', error);
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
