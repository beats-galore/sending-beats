// Device service layer - higher-level device operations
import { uniqBy } from 'lodash';

import type { AudioDeviceInfo } from '../types';
import { audioService } from './audio-service';


export const deviceService = {
  // Device filtering and categorization
  getInputDevices(devices: AudioDeviceInfo[]): AudioDeviceInfo[] {
    return uniqBy(devices, 'id').filter((device) => device.is_input);
  },

  getOutputDevices(devices: AudioDeviceInfo[]): AudioDeviceInfo[] {
    return uniqBy(devices, 'id').filter((device) => device.is_output);
  },

  getDefaultInputDevice(devices: AudioDeviceInfo[]): AudioDeviceInfo | undefined {
    return uniqBy(devices, 'id').find((device) => device.is_input && device.is_default);
  },

  getDefaultOutputDevice(devices: AudioDeviceInfo[]): AudioDeviceInfo | undefined {
    return uniqBy(devices, 'id').find((device) => device.is_output && device.is_default);
  },

  // Device validation
  isValidInputDevice(devices: AudioDeviceInfo[], deviceId: string): boolean {
    return uniqBy(devices, 'id').some((device) => device.id === deviceId && device.is_input);
  },

  isValidOutputDevice(devices: AudioDeviceInfo[], deviceId: string): boolean {
    return uniqBy(devices, 'id').some((device) => device.id === deviceId && device.is_output);
  },

  // Device search and lookup
  findDeviceById(devices: AudioDeviceInfo[], deviceId: string): AudioDeviceInfo | undefined {
    return devices.find((device) => device.id === deviceId);
  },

  findDeviceByName(devices: AudioDeviceInfo[], deviceName: string): AudioDeviceInfo | undefined {
    return uniqBy(devices, 'id').find((device) =>
      device.name.toLowerCase().includes(deviceName.toLowerCase())
    );
  },

  // Device enumeration with error handling
  async getAllDevices(): Promise<AudioDeviceInfo[]> {
    try {
      return await audioService.enumerateAudioDevices();
    } catch (error) {
      console.error('Failed to enumerate audio devices:', error);
      return [];
    }
  },

  async refreshDevices(): Promise<AudioDeviceInfo[]> {
    try {
      return await audioService.refreshAudioDevices();
    } catch (error) {
      console.error('Failed to refresh audio devices:', error);
      return [];
    }
  },
} as const;
