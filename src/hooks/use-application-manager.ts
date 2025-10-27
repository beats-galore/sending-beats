import { invoke } from '@tauri-apps/api/core';
import { useCallback, useState } from 'react';

import type { AvailableApplicationInfo } from '../types/applicationAudio.types';

type UseApplicationManagerReturn = {
  allApplications: AvailableApplicationInfo[];
  isLoading: boolean;
  error: string | null;
  loadAllApplications: () => Promise<void>;
  addApplication: (
    bundleIdentifier: string,
    applicationName: string,
    operatingSystem: string
  ) => Promise<string>;
  updateApplicationName: (bundleIdentifier: string, newApplicationName: string) => Promise<string>;
  removeApplication: (bundleIdentifier: string) => Promise<string>;
  clearError: () => void;
};

export const useApplicationManager = (): UseApplicationManagerReturn => {
  const [allApplications, setAllApplications] = useState<AvailableApplicationInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadAllApplications = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const apps = await invoke<AvailableApplicationInfo[]>('get_all_available_applications');
      setAllApplications(apps);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      console.error('Failed to load all applications:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const addApplication = useCallback(
    async (
      bundleIdentifier: string,
      applicationName: string,
      operatingSystem: string
    ): Promise<string> => {
      setError(null);
      try {
        const result = await invoke<string>('add_audio_application', {
          bundleIdentifier,
          applicationName,
          operatingSystem,
        });
        await loadAllApplications();
        return result;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        throw err;
      }
    },
    [loadAllApplications]
  );

  const updateApplicationName = useCallback(
    async (bundleIdentifier: string, newApplicationName: string): Promise<string> => {
      setError(null);
      try {
        const result = await invoke<string>('update_audio_application_name', {
          bundleIdentifier,
          newApplicationName,
        });
        await loadAllApplications();
        return result;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        throw err;
      }
    },
    [loadAllApplications]
  );

  const removeApplication = useCallback(
    async (bundleIdentifier: string): Promise<string> => {
      setError(null);
      try {
        const result = await invoke<string>('remove_audio_application', {
          bundleIdentifier,
        });
        await loadAllApplications();
        return result;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        throw err;
      }
    },
    [loadAllApplications]
  );

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  return {
    allApplications,
    isLoading,
    error,
    loadAllApplications,
    addApplication,
    updateApplicationName,
    removeApplication,
    clearError,
  };
};
