import { Group, Text } from '@mantine/core';
import { useCallback } from 'react';

import { useChannelsData } from '../../../hooks';
import { sourcesOf, useBusStore } from '../../../stores/bus-store';
import { color } from '../../../theme/tokens';
import { SourceTile } from './SourceTile';

type SourceTilesProps = {
  /** The destination these tiles route into, by device identifier */
  deviceId: string;
};

/** Which sources reach one destination. Click a tile to add or remove it. */
export const SourceTiles = ({ deviceId }: SourceTilesProps) => {
  const { channels } = useChannelsData();
  const buses = useBusStore((state) => state.buses);
  const setOutputSources = useBusStore((state) => state.setOutputSources);

  const current = sourcesOf(buses, deviceId);

  const toggle = useCallback(
    (deviceIdentifier: string) => {
      const next = current.includes(deviceIdentifier)
        ? current.filter((id) => id !== deviceIdentifier)
        : [...current, deviceIdentifier];

      void setOutputSources(deviceId, next);
    },
    [current, deviceId, setOutputSources]
  );

  if (channels.length === 0) {
    return (
      <Text size="3xs" c={color.textFaintest}>
        No sources patched
      </Text>
    );
  }

  return (
    // Scrolls rather than clips: a named tile is wide enough that a handful of
    // sources runs past the card, and a tile you cannot reach cannot be routed.
    <Group gap="3xs" wrap="nowrap" style={{ overflowX: 'auto', minWidth: 0 }}>
      {channels.map((channel, index) => (
        <SourceTile
          key={channel.id}
          channelId={channel.id}
          index={index}
          name={channel.name}
          sources={current}
          onToggle={toggle}
        />
      ))}
    </Group>
  );
};
