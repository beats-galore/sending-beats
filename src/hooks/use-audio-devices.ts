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
    loadDevices,
    refreshDevices,
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

  useEffect(() => {
    console.log('[use-audio-devices] devices changed', devices);
  }, [devices]);

  useEffect(() => {
    console.log('[use-audio-devices] isLoading changed', isLoading);
  }, [isLoading]);

  useEffect(() => {
    console.log('[use-audio-devices] error changed', error);
  }, [error]);

  useEffect(() => {
    console.log('[use-audio-devices] inputDevices changed', inputDevices);
  }, [inputDevices]);

  useEffect(() => {
    console.log('[use-audio-devices] outputDevices changed', outputDevices);
  }, [outputDevices]);

  useEffect(() => {
    console.log('[use-audio-devices] defaultInputDevice changed', defaultInputDevice);
  }, [defaultInputDevice]);

  useEffect(() => {
    console.log('[use-audio-devices] defaultOutputDevice changed', defaultOutputDevice);
  }, [defaultOutputDevice]);

  useEffect(() => {
    console.log('[use-audio-devices] loadDevices changed');
  }, [loadDevices]);

  useEffect(() => {
    console.log('[use-audio-devices] refreshDevices changed');
  }, [refreshDevices]);

  useEffect(() => {
    console.log('[use-audio-devices] findDevice changed');
  }, [findDevice]);

  useEffect(() => {
    console.log('[use-audio-devices] isValidInput changed');
  }, [isValidInput]);

  useEffect(() => {
    console.log('[use-audio-devices] isValidOutput changed');
  }, [isValidOutput]);

  useEffect(() => {
    console.log('[use-audio-devices] clearError changed');
  }, [clearError]);

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
      refreshDevices,
      clearError,
      getDeviceById,
      getDeviceName,
      validateInputDevice,
      validateOutputDevice,
    ]
  );
};
