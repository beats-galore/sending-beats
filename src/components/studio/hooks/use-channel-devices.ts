import { useMemo } from 'react';

import { useChannelsData } from '../../../hooks';
import { useMixerStore } from '../../../stores/mixer-store';

export type ChannelDevice = {
  channelId: number;
  /** Position in the source column, which is what the strip is numbered by */
  index: number;
  /** The channel's name, or what is patched into it when it has none */
  name: string;
  /** What the mixing layer routes this channel by, null when nothing is patched */
  deviceIdentifier: string | null;
};

/**
 * Every source strip with the identifier the router knows it by.
 *
 * Routing is keyed by device identifier while the interface is keyed by channel
 * number, so anything reading a bus back has to cross the two. Resolving it
 * once here keeps that join out of the components that draw it.
 */
export const useChannelDevices = (): ChannelDevice[] => {
  const { channels } = useChannelsData();
  const configuredDevices = useMixerStore((state) => state.activeSession?.configuredDevices);

  return useMemo(
    () =>
      channels.map((channel, index) => {
        const device = configuredDevices?.find(
          (candidate) => candidate.channelNumber === channel.id && candidate.isInput
        );

        return {
          channelId: channel.id,
          index,
          name: channel.name || device?.deviceName || 'No input',
          deviceIdentifier: device?.deviceIdentifier ?? null,
        };
      }),
    [channels, configuredDevices]
  );
};
