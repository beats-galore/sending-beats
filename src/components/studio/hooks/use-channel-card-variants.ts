import { useEffect, useMemo } from 'react';

import { useConfigurationStore } from '../../../stores/mixer-store';
import { useNowPlayingStore } from '../../../stores/now-playing-store';
import { bundleIdFromDeviceIdentifier } from '../../../types/now-playing.types';
import type { ChannelCardVariant } from '../patch/patch-geometry';

/**
 * Which card each channel is drawn as, keyed by channel number.
 *
 * Every application source gets the taller card. Metadata no longer comes only
 * from the two players with a scripting dictionary — anything that publishes to
 * the system now-playing session can fill the readout — so which applications
 * have something to show is not knowable ahead of time. One that never does
 * shows "Nothing playing" rather than resizing the moment it starts.
 *
 * This is also where the now-playing watcher is attached, so the subscription
 * is made once for the canvas rather than once per node.
 */
export const useChannelCardVariants = (): Record<number, ChannelCardVariant> => {
  const subscribe = useNowPlayingStore((state) => state.subscribe);
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
      variants[device.channelNumber] = bundleIdFromDeviceIdentifier(device.deviceIdentifier)
        ? 'app'
        : 'device';
    }

    return variants;
  }, [activeSession]);
};
