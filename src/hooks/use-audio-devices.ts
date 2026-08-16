// Custom hook for audio device management
import { useCallback, useEffect, useMemo } from 'react';

import { useAudioDeviceStore } from '../stores';

import type { AudioDeviceInfo } from '../types';

export const useAudioDevices = () => {
  const devices = useAudioDeviceStore((state) => state.devices);
  const isLoading = useAudioDeviceStore((state) => state.isLoading);
  const error = useAudioDeviceStore((state) => state.error);
  const inputDevices = useAudioDeviceStore((state) => state.inputDevices);
  const outputDevices = useAudioDeviceStore((state) => state.outputDevices);
  const defaultInputDevice = useAudioDeviceStore((state) => state.defaultInputDevice);
  const defaultOutputDevice = useAudioDeviceStore((state) => state.defaultOutputDevice);
  const disconnectedDeviceIds = useAudioDeviceStore((state) => state.disconnectedDeviceIds);
  const loadDevices = useAudioDeviceStore((state) => state.loadDevices);
  const refreshDevices = useAudioDeviceStore((state) => state.refreshDevices);
  const subscribeToHotplug = useAudioDeviceStore((state) => state.subscribeToHotplug);
  const findDevice = useAudioDeviceStore((state) => state.findDevice);
  const isValidInput = useAudioDeviceStore((state) => state.isValidInput);
  const isValidOutput = useAudioDeviceStore((state) => state.isValidOutput);
  const clearError = useAudioDeviceStore((state) => state.clearError);

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
