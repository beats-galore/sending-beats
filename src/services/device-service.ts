// Device service layer - higher-level device operations
import { AudioDeviceInfo } from '../types';
import { audioService } from './audio-service';

export const deviceService = {
  // Device filtering and categorization
  getInputDevices(devices: AudioDeviceInfo[]): AudioDeviceInfo[] {
    return devices.filter(device => device.is_input);
  },

  getOutputDevices(devices: AudioDeviceInfo[]): AudioDeviceInfo[] {
    return devices.filter(device => device.is_output);
  },

  getDefaultInputDevice(devices: AudioDeviceInfo[]): AudioDeviceInfo | undefined {
    return devices.find(device => device.is_input && device.is_default);
  },

  getDefaultOutputDevice(devices: AudioDeviceInfo[]): AudioDeviceInfo | undefined {
    return devices.find(device => device.is_output && device.is_default);
  },

  // Device validation
  isValidInputDevice(devices: AudioDeviceInfo[], deviceId: string): boolean {
    return devices.some(device => 
      device.id === deviceId && device.is_input
    );
  },

  isValidOutputDevice(devices: AudioDeviceInfo[], deviceId: string): boolean {
    return devices.some(device => 
      device.id === deviceId && device.is_output
    );
  },

  // Device search and lookup
  findDeviceById(devices: AudioDeviceInfo[], deviceId: string): AudioDeviceInfo | undefined {
    return devices.find(device => device.id === deviceId);
  },

  findDeviceByName(devices: AudioDeviceInfo[], deviceName: string): AudioDeviceInfo | undefined {
    return devices.find(device => 
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
  }
} as const;