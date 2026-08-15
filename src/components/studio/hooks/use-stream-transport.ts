import { useCallback } from 'react';

import { useStreamingControls, useStreamingStatus } from '../../../hooks';
import {
  selectedCastConfiguration,
  useCastConfigurationStore,
} from '../../../stores/cast-configuration-store';

/**
 * Stream status plus a single go-live / cut-the-feed toggle.
 *
 * The status poll and the transport commands are separate hooks in the data
 * layer; the interface only ever wants them together.
 */
export const useStreamTransport = (pollingInterval?: number) => {
  const { status, isLoading, error, actions } = useStreamingStatus(pollingInterval);
  const { state: controlState, actions: controls } = useStreamingControls();
  const station = useCastConfigurationStore(selectedCastConfiguration);

  const isLive = Boolean(status?.is_streaming);
  const isBusy = controlState.isStarting || controlState.isStopping || controlState.isConnecting;

  const toggle = useCallback(async () => {
    if (isBusy) {
      return;
    }

    if (isLive) {
      await controls.stopStreaming();
    } else {
      // Nothing to go live to. The button is only reachable with a station
      // selected, so this is the case where the last one was deleted.
      if (!station) {
        return;
      }

      // The station is named rather than described: the backend reads its
      // details and its keychain password itself, so what goes on air is what is
      // stored rather than a copy the interface was holding.
      await controls.startStreaming(station.id);
    }

    await actions.refreshStatus();
  }, [isBusy, isLive, controls, actions, station]);

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
