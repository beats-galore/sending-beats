import { useCallback } from 'react';

import { useStreamingControls, useStreamingStatus } from '../../../hooks';
import { useStudioStore } from '../../../stores/studio-store';

/**
 * Stream status plus a single go-live / cut-the-feed toggle.
 *
 * The status poll and the transport commands are separate hooks in the data
 * layer; the interface only ever wants them together.
 */
export const useStreamTransport = (pollingInterval?: number) => {
  const { status, isLoading, error, actions } = useStreamingStatus(pollingInterval);
  const { state: controlState, actions: controls } = useStreamingControls();
  const settings = useStudioStore((state) => state.stream);

  const isLive = Boolean(status?.is_streaming);
  const isBusy = controlState.isStarting || controlState.isStopping || controlState.isConnecting;

  const toggle = useCallback(async () => {
    if (isBusy) {
      return;
    }

    if (isLive) {
      await controls.stopStreaming();
    } else {
      // Going live always re-initialises first: the target is edited in CAST and
      // the engine only learns about it when the connection is established.
      await controls.initialize({
        server_host: settings.host,
        server_port: settings.port,
        mount_point: settings.mount,
        password: settings.password,
        stream_name: settings.mount.replace(/^\//, '') || 'live',
        bitrate: settings.bitrate,
      });
      await controls.startStreaming();
    }

    await actions.refreshStatus();
  }, [isBusy, isLive, controls, actions, settings]);

  return {
    status,
    isLoading,
    isLive,
    isBusy,
    error: error ?? controlState.error,
    uptimeSeconds: status?.uptime_seconds ?? 0,
    toggle,
    controls,
    statusActions: actions,
  };
};
