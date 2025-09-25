import { invoke } from '@tauri-apps/api/core';
import React, { useState, useEffect } from 'react';

import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';

type SaveAsNewConfigurationProps = {
  onConfigurationCreated?: (config: AudioMixerConfiguration) => void;
  onCancel?: () => void;
  className?: string;
}

export const SaveAsNewConfiguration: React.FC<SaveAsNewConfigurationProps> = ({
  onConfigurationCreated,
  onCancel,
  className = '',
}) => {
  const [activeSession, setActiveSession] = useState<AudioMixerConfiguration | null>(null);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isVisible, setIsVisible] = useState(false);

  // Load active session on mount
  useEffect(() => {
    loadActiveSession();
  }, []);

  const loadActiveSession = async () => {
    try {
      const session = await invoke<AudioMixerConfiguration | null>('get_active_session_configuration');
      setActiveSession(session);

      // Pre-fill the form with session data
      if (session) {
        setName(session.name.replace(' (Session)', '') || '');
        setDescription(session.description || '');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load active session');
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!name.trim()) {
      setError('Configuration name is required');
      return;
    }

    try {
      setIsLoading(true);
      setError(null);

      const newConfig = await invoke<AudioMixerConfiguration>('save_session_as_new_reusable', {
        name: name.trim(),
        description: description.trim() || undefined,
      });

      onConfigurationCreated?.(newConfig);
      handleCancel(); // Close the form
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save configuration');
    } finally {
      setIsLoading(false);
    }
  };

  const handleCancel = () => {
    setIsVisible(false);
    setName('');
    setDescription('');
    setError(null);
    onCancel?.();
  };

  const handleShow = () => {
    setIsVisible(true);
  };

  if (!activeSession) {
    return (
      <div className={`p-4 bg-gray-50 border border-gray-200 rounded-lg ${className}`}>
        <p className="text-sm text-gray-600">
          No active session found. Start a session to save it as a reusable configuration.
        </p>
      </div>
    );
  }

  if (!isVisible) {
    return (
      <div className={className}>
        <button
          onClick={handleShow}
          className="w-full px-4 py-2 text-sm font-medium text-green-700 bg-green-50 hover:bg-green-100 border border-green-200 rounded-md transition-colors"
        >
          Save as New Configuration
        </button>
      </div>
    );
  }

  return (
    <div className={`space-y-4 ${className}`}>
      {/* Current Session Info */}
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-3">
        <h4 className="font-medium text-blue-800 mb-1">Saving Current Session</h4>
        <p className="text-sm text-blue-700">{activeSession.name}</p>
        {activeSession.description && (
          <p className="text-xs text-blue-600 mt-1">{activeSession.description}</p>
        )}
      </div>

      {/* Form */}
      <form onSubmit={handleSave} className="space-y-4">
        <div>
          <label htmlFor="config-name" className="block text-sm font-medium text-gray-700 mb-1">
            Configuration Name *
          </label>
          <input
            id="config-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-green-500 focus:border-green-500"
            placeholder="Enter configuration name"
            required
            maxLength={100}
          />
        </div>

        <div>
          <label htmlFor="config-description" className="block text-sm font-medium text-gray-700 mb-1">
            Description
          </label>
          <textarea
            id="config-description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={3}
            className="block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-green-500 focus:border-green-500"
            placeholder="Optional description"
            maxLength={500}
          />
          <p className="text-xs text-gray-500 mt-1">
            {description.length}/500 characters
          </p>
        </div>

        {/* Action Buttons */}
        <div className="flex gap-3">
          <button
            type="submit"
            disabled={isLoading || !name.trim()}
            className={`flex-1 px-4 py-2 text-sm font-medium rounded-md border transition-colors ${
              !isLoading && name.trim()
                ? 'bg-green-600 hover:bg-green-700 text-white border-green-600'
                : 'bg-gray-100 text-gray-400 border-gray-300 cursor-not-allowed'
            }`}
          >
            {isLoading ? 'Saving...' : 'Save Configuration'}
          </button>

          <button
            type="button"
            onClick={handleCancel}
            disabled={isLoading}
            className="px-4 py-2 text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 border border-gray-300 rounded-md transition-colors"
          >
            Cancel
          </button>
        </div>
      </form>

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