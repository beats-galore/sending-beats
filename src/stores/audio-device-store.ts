// Zustand store for audio device state management
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import { AudioDeviceInfo } from '../types';
import { deviceService } from '../services';

type AudioDeviceStore = {
  // State
  devices: AudioDeviceInfo[];
  isLoading: boolean;
  error: string | null;
  
  // Computed values
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
    error: null,
    
    // Computed values (getters)
    get inputDevices() {
      return deviceService.getInputDevices(get().devices);
    },
    
    get outputDevices() {
      return deviceService.getOutputDevices(get().devices);
    },
    
    get defaultInputDevice() {
      return deviceService.getDefaultInputDevice(get().devices) || null;
    },
    
    get defaultOutputDevice() {
      return deviceService.getDefaultOutputDevice(get().devices) || null;
    },

    // Load devices initially
    loadDevices: async () => {
      set({ isLoading: true, error: null });
      
      try {
        const devices = await deviceService.getAllDevices();
        set({ 
          devices,
          isLoading: false,
          error: null 
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ 
          isLoading: false,
          error: `Failed to load audio devices: ${errorMessage}`
        });
      }
    },

    // Refresh devices
    refreshDevices: async () => {
      set({ isLoading: true, error: null });
      
      try {
        const devices = await deviceService.refreshDevices();
        set({ 
          devices,
          isLoading: false,
          error: null 
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ 
          isLoading: false,
          error: `Failed to refresh audio devices: ${errorMessage}`
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
    }
  }))
);