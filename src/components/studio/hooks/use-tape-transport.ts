import { useCallback, useEffect, useState } from 'react';

import { useRecording } from '../../../hooks';

import type { RecordingConfig } from '../../../types/audio.types';

/**
 * The tape: its configuration, what it is doing, and one control to arm or stop it.
 *
 * Recording needs a config to start from. The saved configs are the source of
 * truth; if there are none yet the backend's default is pulled in so the
 * transport is usable on a fresh install.
 */
export const useTapeTransport = () => {
  const { status, configs, history, error, actions } = useRecording();
  const [config, setConfig] = useState<RecordingConfig | null>(null);

  useEffect(() => {
    if (config) {
      return;
    }
    if (configs.length > 0) {
      setConfig(configs[0]);
      return;
    }
    void actions
      .createDefaultConfig()
      .then(setConfig)
      .catch(() => undefined);
  }, [config, configs, actions]);

  const session = status?.current_session;
  const isRecording = Boolean(status?.is_recording);

  const toggle = useCallback(async () => {
    if (isRecording) {
      await actions.stopRecording();
      return;
    }
    if (config) {
      await actions.startRecording(config);
    }
  }, [isRecording, config, actions]);

  const updateConfig = useCallback(
    (patch: Partial<RecordingConfig>) => {
      if (!config) {
        return;
      }
      const next = { ...config, ...patch };
      setConfig(next);
      void actions.saveConfig(next);
    },
    [config, actions]
  );

  return {
    config,
    updateConfig,
    status,
    history,
    error,
    isRecording,
    elapsedSeconds: session?.duration_seconds ?? 0,
    fileSizeBytes: session?.file_size_bytes ?? 0,
    filePath: session?.current_file_path ?? null,
    availableSpaceGb: status?.available_space_gb ?? 0,
    totalRecordings: status?.total_recordings ?? 0,
    toggle,
    actions,
  };
};
