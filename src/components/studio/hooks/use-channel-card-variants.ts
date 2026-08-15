import { useEffect, useMemo } from 'react';

import { useConfigurationStore } from '../../../stores/mixer-store';
import { useNowPlayingStore } from '../../../stores/now-playing-store';
import { bundleIdFromDeviceIdentifier } from '../../../types/now-playing.types';
import type { ChannelCardVariant } from '../patch/patch-geometry';

/**
 * Which card each channel is drawn as, keyed by channel number.
 *
 * Only an application the backend can read a track from earns the taller card;
 * one it cannot read is captured the same way but has nothing to show, so it
 * stays a plain device card.
 *
 * This is also where the now-playing watcher is attached, so the subscription
 * is made once for the canvas rather than once per node.
 */
export const useChannelCardVariants = (): Record<number, ChannelCardVariant> => {
  const subscribe = useNowPlayingStore((state) => state.subscribe);
  const supportedBundleIds = useNowPlayingStore((state) => state.supportedBundleIds);
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
        bundleId && supportedBundleIds.includes(bundleId) ? 'app' : 'device';
    }

    return variants;
  }, [activeSession, supportedBundleIds]);
};
