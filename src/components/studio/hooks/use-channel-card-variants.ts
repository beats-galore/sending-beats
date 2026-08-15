import { useEffect, useMemo } from 'react';

import { useConfigurationStore } from '../../../stores/mixer-store';
import { useNowPlayingStore } from '../../../stores/now-playing-store';
import { bundleIdFromDeviceIdentifier } from '../../../types/now-playing.types';
import type { ChannelCardVariant } from '../patch/patch-geometry';

/**
 * Which card each channel is drawn as, keyed by channel number.
 *
 * An application source earns the taller card by having reported a track, not
 * by being an application. Plenty never report one — Serato is captured like
 * any other source but never announces what it is playing — and giving those a
 * readout only to leave it empty states something false about a deck that is
 * audibly running.
 *
 * The cost is that a source resizes once, when its first track lands. That
 * beats a permanent empty row on every source that will never fill one.
 *
 * This is also where the now-playing watcher is attached, so the subscription
 * is made once for the canvas rather than once per node.
 */
export const useChannelCardVariants = (): Record<number, ChannelCardVariant> => {
  const subscribe = useNowPlayingStore((state) => state.subscribe);
  const reportedBundleIds = useNowPlayingStore((state) => state.reportedBundleIds);
  const { activeSession } = useConfigurationStore();

  useEffect(() => {
    void subscribe();
  }, [subscribe]);

  return useMemo(() => {
    const variants: Record<number, ChannelCardVariant> = {};

    for (const device of activeSession?.configuredDevices ?? []) {
      if (!device.isInput) {
        continue;
      }
      const bundleId = bundleIdFromDeviceIdentifier(device.deviceIdentifier);
      variants[device.channelNumber] =
        bundleId && reportedBundleIds.includes(bundleId) ? 'app' : 'device';
    }

    return variants;
  }, [activeSession, reportedBundleIds]);
};
