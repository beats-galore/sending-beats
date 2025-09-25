import { invoke } from '@tauri-apps/api/core';
import React, { useState, useEffect } from 'react';

import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';

type ConfigurationSelectorProps = {
  onConfigurationSelect?: (configId: string) => void;
  className?: string;
}

export const ConfigurationSelector: React.FC<ConfigurationSelectorProps> = ({
  onConfigurationSelect,
  className = '',
}) => {
  const [reusableConfigs, setReusableConfigs] = useState<AudioMixerConfiguration[]>([]);
  const [activeSession, setActiveSession] = useState<AudioMixerConfiguration | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load configurations on mount
  useEffect(() => {
    loadConfigurations();
  }, []);

  const loadConfigurations = async () => {
    setIsLoading(true);
    setError(null);

    try {
      // Load both reusable configurations and active session
      const [reusable, active] = await Promise.all([
        invoke<AudioMixerConfiguration[]>('get_reusable_configurations'),
        invoke<AudioMixerConfiguration | null>('get_active_session_configuration'),
      ]);

      setReusableConfigs(reusable);
      setActiveSession(active);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load configurations');
    } finally {
      setIsLoading(false);
    }
  };

  const handleConfigurationSelect = async (configId: string) => {
    if (!configId) return;

    try {
      setIsLoading(true);

      // Create a new session from the selected reusable configuration
      const newSession = await invoke<AudioMixerConfiguration>('create_session_from_reusable', {
        reusableId: configId,
        sessionName: undefined, // Let it auto-generate the name
      });

      setActiveSession(newSession);
      onConfigurationSelect?.(newSession.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to select configuration');
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading && reusableConfigs.length === 0) {
    return (
      <div className={`flex items-center justify-center p-4 ${className}`}>
        <div className="text-sm text-gray-500">Loading configurations...</div>
      </div>
    );
  }

  return (
    <div className={`space-y-4 ${className}`}>
      {/* Active Session Display */}
      {activeSession && (
        <div className="bg-green-50 border border-green-200 rounded-lg p-3">
          <h4 className="font-medium text-green-800 mb-1">Active Session</h4>
          <p className="text-sm text-green-700">{activeSession.name}</p>
          {activeSession.description && (
            <p className="text-xs text-green-600 mt-1">{activeSession.description}</p>
          )}
        </div>
      )}

      {/* Configuration Selector */}
      <div>
        <label htmlFor="config-select" className="block text-sm font-medium text-gray-700 mb-2">
          Load Reusable Configuration
        </label>

        <select
          id="config-select"
          className="block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          onChange={(e) => handleConfigurationSelect(e.target.value)}
          disabled={isLoading}
        >
          <option value="">Select a configuration...</option>
          {reusableConfigs.map((config) => (
            <option key={config.id} value={config.id}>
              {config.name}
              {config.description && ` - ${config.description}`}
            </option>
          ))}
        </select>
      </div>

      {/* Error Display */}
      {error && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-3">
          <p className="text-sm text-red-700">{error}</p>
          <button
            onClick={() => setError(null)}
            className="mt-2 text-xs text-red-600 hover:text-red-800 underline"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Empty State */}
      {reusableConfigs.length === 0 && !isLoading && (
        <div className="text-center p-4 text-gray-500">
          <p className="text-sm">No reusable configurations found.</p>
          <p className="text-xs mt-1">Create a configuration by saving your current session.</p>
        </div>
      )}
    </div>
  );
};