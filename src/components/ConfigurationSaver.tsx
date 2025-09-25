import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api';
import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';

interface ConfigurationSaverProps {
  onConfigurationSaved?: (config: AudioMixerConfiguration) => void;
  className?: string;
}

export const ConfigurationSaver: React.FC<ConfigurationSaverProps> = ({
  onConfigurationSaved,
  className = '',
}) => {
  const [activeSession, setActiveSession] = useState<AudioMixerConfiguration | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  // Load active session on mount
  useEffect(() => {
    loadActiveSession();
  }, []);

  const loadActiveSession = async () => {
    try {
      const session = await invoke<AudioMixerConfiguration | null>('get_active_session_configuration');
      setActiveSession(session);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load active session');
    }
  };

  const handleSaveToReusable = async () => {
    if (!activeSession?.reusableConfigurationId) {
      setError('Active session is not linked to a reusable configuration');
      return;
    }

    try {
      setIsLoading(true);
      setError(null);
      setSuccessMessage(null);

      await invoke('save_session_to_reusable');

      setSuccessMessage('Configuration saved successfully!');
      setTimeout(() => setSuccessMessage(null), 3000);

      // Reload active session to get updated data
      await loadActiveSession();

      if (activeSession && onConfigurationSaved) {
        onConfigurationSaved(activeSession);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save configuration');
    } finally {
      setIsLoading(false);
    }
  };

  // Auto-clear error messages after 5 seconds
  useEffect(() => {
    if (error) {
      const timeout = setTimeout(() => setError(null), 5000);
      return () => clearTimeout(timeout);
    }
  }, [error]);

  if (!activeSession) {
    return (
      <div className={`p-4 bg-gray-50 border border-gray-200 rounded-lg ${className}`}>
        <p className="text-sm text-gray-600">
          No active session found. Select a reusable configuration to start a session.
        </p>
      </div>
    );
  }

  const canSaveToReusable = activeSession.reusableConfigurationId != null;

  return (
    <div className={`space-y-4 ${className}`}>
      {/* Active Session Info */}
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-3">
        <h4 className="font-medium text-blue-800 mb-1">Current Session</h4>
        <p className="text-sm text-blue-700">{activeSession.name}</p>
        {activeSession.description && (
          <p className="text-xs text-blue-600 mt-1">{activeSession.description}</p>
        )}
        {canSaveToReusable && (
          <p className="text-xs text-blue-600 mt-2">
            ✓ Linked to reusable configuration - changes can be saved
          </p>
        )}
      </div>

      {/* Save Actions */}
      <div className="space-y-3">
        {/* Save to Existing Reusable Configuration */}
        <button
          onClick={handleSaveToReusable}
          disabled={!canSaveToReusable || isLoading}
          className={`w-full px-4 py-2 text-sm font-medium rounded-md border transition-colors ${
            canSaveToReusable && !isLoading
              ? 'bg-blue-600 hover:bg-blue-700 text-white border-blue-600'
              : 'bg-gray-100 text-gray-400 border-gray-300 cursor-not-allowed'
          }`}
        >
          {isLoading ? 'Saving...' : 'Save to Reusable Configuration'}
        </button>

        {!canSaveToReusable && (
          <p className="text-xs text-gray-500 text-center">
            This session is not linked to a reusable configuration.
            Use "Save as New" to create a reusable configuration.
          </p>
        )}
      </div>

      {/* Success Message */}
      {successMessage && (
        <div className="bg-green-50 border border-green-200 rounded-lg p-3">
          <p className="text-sm text-green-700">✓ {successMessage}</p>
        </div>
      )}

      {/* Error Message */}
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
    </div>
  );
};