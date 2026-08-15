// Custom hook for audio device management
import { useCallback, useEffect, useMemo } from 'react';

import { useAudioDeviceStore } from '../stores';

import type { AudioDeviceInfo } from '../types';

export const useAudioDevices = () => {
  const {
    devices,
    isLoading,
    error,
    inputDevices,
    outputDevices,
    defaultInputDevice,
    defaultOutputDevice,
    disconnectedDeviceIds,
    loadDevices,
    refreshDevices,
    subscribeToHotplug,
    findDevice,
    isValidInput,
    isValidOutput,
    clearError,
  } = useAudioDeviceStore();

  // Load devices on mount
  useEffect(() => {
    console.debug('🎧 useAudioDevices: Loading devices on mount...');
    loadDevices();
  }, [loadDevices]);

  // The backend pushes the list on every hardware change, so this is the only
  // thing keeping it current after the initial load.
  useEffect(() => {
    void subscribeToHotplug();
  }, [subscribeToHotplug]);

  // Helper functions
  const getDeviceById = useCallback(
    (deviceId: string): AudioDeviceInfo | null => {
      return findDevice(deviceId);
    },
    [findDevice]
  );

  const getDeviceName = useCallback(
    (deviceId: string): string => {
      const device = findDevice(deviceId);
      return device?.name || 'Unknown Device';
    },
    [findDevice]
  );

  const validateInputDevice = useCallback(
    (deviceId: string): boolean => {
      return isValidInput(deviceId);
    },
    [isValidInput]
  );

  const validateOutputDevice = useCallback(
    (deviceId: string): boolean => {
      return isValidOutput(deviceId);
    },
    [isValidOutput]
  );

  return useMemo(
    () => ({
      // State
      devices,
      isLoading,
      error,

      // Categorized devices
      inputDevices,
      outputDevices,
      defaultInputDevice,
      defaultOutputDevice,
      disconnectedDeviceIds,

      // Actions
      refreshDevices,
      clearError,

      // Helper functions
      getDeviceById,
      getDeviceName,
      validateInputDevice,
      validateOutputDevice,
    }),
    [
      devices,
      isLoading,
      error,
      inputDevices,
      outputDevices,
      defaultInputDevice,
      defaultOutputDevice,
      disconnectedDeviceIds,
      refreshDevices,
      clearError,
      getDeviceById,
      getDeviceName,
      validateInputDevice,
      validateOutputDevice,
    ]
  );
};
