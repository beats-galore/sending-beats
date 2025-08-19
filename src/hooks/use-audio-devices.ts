// Custom hook for audio device management
import { useEffect } from 'react';
import { useAudioDeviceStore } from '../stores';
import { AudioDeviceInfo } from '../types';

export const useAudioDevices = () => {
  const {
    devices,
    isLoading,
    error,
    inputDevices,
    outputDevices,
    defaultInputDevice,
    defaultOutputDevice,
    loadDevices,
    refreshDevices,
    findDevice,
    isValidInput,
    isValidOutput,
    clearError
  } = useAudioDeviceStore();

  // Load devices on mount
  useEffect(() => {
    console.log('🎧 useAudioDevices: Loading devices on mount...');
    loadDevices();
  }, [loadDevices]);

  // Helper functions
  const getDeviceById = (deviceId: string): AudioDeviceInfo | null => {
    return findDevice(deviceId);
  };

  const getDeviceName = (deviceId: string): string => {
    const device = findDevice(deviceId);
    return device?.name || 'Unknown Device';
  };

  const validateInputDevice = (deviceId: string): boolean => {
    return isValidInput(deviceId);
  };

  const validateOutputDevice = (deviceId: string): boolean => {
    return isValidOutput(deviceId);
  };


  return {
    // State
    devices,
    isLoading,
    error,
    
    // Categorized devices
    inputDevices,
    outputDevices,
    defaultInputDevice,
    defaultOutputDevice,
    
    // Actions
    refreshDevices,
    clearError,
    
    // Helper functions
    getDeviceById,
    getDeviceName,
    validateInputDevice,
    validateOutputDevice,
  };
};