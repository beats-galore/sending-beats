// Zustand store for audio device state management
import isEqual from 'fast-deep-equal';
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

import { audioService, deviceService } from '../services';

import type { AudioDeviceInfo } from '../types';

type AudioDeviceStore = {
  // State
  devices: AudioDeviceInfo[];
  isLoading: boolean;
  initialLoadCompleted: boolean;
  error: string | null;

  // Computed values (these will be updated when devices change)
  inputDevices: AudioDeviceInfo[];
  outputDevices: AudioDeviceInfo[];
  defaultInputDevice: AudioDeviceInfo | null;
  defaultOutputDevice: AudioDeviceInfo | null;

  // Actions
  loadDevices: () => Promise<void>;
  refreshDevices: () => Promise<void>;
  setError: (error: string | null) => void;
  clearError: () => void;

  // Device lookup
  findDevice: (deviceId: string) => AudioDeviceInfo | null;
  isValidInput: (deviceId: string) => boolean;
  isValidOutput: (deviceId: string) => boolean;
};

export const useAudioDeviceStore = create<AudioDeviceStore>()(
  subscribeWithSelector((set, get) => ({
    // Initial state
    devices: [],
    isLoading: false,
    initialLoadCompleted: false,
    error: null,

    // Computed values (updated whenever devices change)
    inputDevices: [],
    outputDevices: [],
    defaultInputDevice: null,
    defaultOutputDevice: null,

    // Load devices initially
    loadDevices: async () => {
      const { isLoading, initialLoadCompleted } = get();
      if (isLoading || initialLoadCompleted) {
        return;
      }
      console.debug('🎧 Loading audio devices...');
      set({ isLoading: true, error: null });

      try {
        const devices = await audioService.enumerateAudioDevices();
        console.debug('🎧 Loaded devices:', {
          total: devices.length,
          input: devices.filter((d) => d.is_input).length,
          output: devices.filter((d) => d.is_output).length,
          devices: devices.map((d) => ({
            id: d.id,
            name: d.name,
            is_input: d.is_input,
            is_output: d.is_output,
          })),
        });

        // Update devices and compute derived values with change detection
        const inputDevices = deviceService.getInputDevices(devices);
        const outputDevices = deviceService.getOutputDevices(devices);
        const defaultInputDevice = deviceService.getDefaultInputDevice(devices) ?? null;
        const defaultOutputDevice = deviceService.getDefaultOutputDevice(devices) ?? null;

        set((state) => {
          const updates: Partial<AudioDeviceStore> = {};

          if (!isEqual(state.devices, devices)) {
            updates.devices = devices;
          }
          if (!isEqual(state.inputDevices, inputDevices)) {
            updates.inputDevices = inputDevices;
          }
          if (!isEqual(state.outputDevices, outputDevices)) {
            updates.outputDevices = outputDevices;
          }
          if (!isEqual(state.defaultInputDevice, defaultInputDevice)) {
            updates.defaultInputDevice = defaultInputDevice;
          }
          if (!isEqual(state.defaultOutputDevice, defaultOutputDevice)) {
            updates.defaultOutputDevice = defaultOutputDevice;
          }

          return {
            ...updates,
            isLoading: false,
            initialLoadCompleted: true,
            error: null,
          };
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        console.error('❌ Failed to load audio devices:', errorMessage);
        set({
          isLoading: false,
          initialLoadCompleted: true,
          error: `Failed to load audio devices: ${errorMessage}`,
        });
      }
    },

    // Refresh devices
    refreshDevices: async () => {
      console.debug('🔄 Refreshing audio devices...');
      set({ isLoading: true, error: null });

      try {
        const devices = await audioService.refreshAudioDevices();
        console.debug('🔄 Refreshed devices:', {
          total: devices.length,
          input: devices.filter((d) => d.is_input).length,
          output: devices.filter((d) => d.is_output).length,
        });

        // Update devices and compute derived values with change detection
        const inputDevices = deviceService.getInputDevices(devices);
        const outputDevices = deviceService.getOutputDevices(devices);
        const defaultInputDevice = deviceService.getDefaultInputDevice(devices) || null;
        const defaultOutputDevice = deviceService.getDefaultOutputDevice(devices) || null;

        set((state) => {
          const updates: Partial<AudioDeviceStore> = {};

          if (!isEqual(state.devices, devices)) {
            updates.devices = devices;
          }
          if (!isEqual(state.inputDevices, inputDevices)) {
            updates.inputDevices = inputDevices;
          }
          if (!isEqual(state.outputDevices, outputDevices)) {
            updates.outputDevices = outputDevices;
          }
          if (!isEqual(state.defaultInputDevice, defaultInputDevice)) {
            updates.defaultInputDevice = defaultInputDevice;
          }
          if (!isEqual(state.defaultOutputDevice, defaultOutputDevice)) {
            updates.defaultOutputDevice = defaultOutputDevice;
          }

          return {
            ...updates,
            isLoading: false,
            error: null,
          };
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        console.error('❌ Failed to refresh audio devices:', errorMessage);
        set({
          isLoading: false,
          error: `Failed to refresh audio devices: ${errorMessage}`,
        });
      }
    },

    // Error handling
    setError: (error: string | null) => {
      set({ error });
    },

    clearError: () => {
      set({ error: null });
    },

    // Device lookup methods
    findDevice: (deviceId: string) => {
      return deviceService.findDeviceById(get().devices, deviceId) || null;
    },

    isValidInput: (deviceId: string) => {
      return deviceService.isValidInputDevice(get().devices, deviceId);
    },

    isValidOutput: (deviceId: string) => {
      return deviceService.isValidOutputDevice(get().devices, deviceId);
    },
  }))
);
