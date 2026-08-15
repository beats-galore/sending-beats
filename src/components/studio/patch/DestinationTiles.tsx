import { Group } from '@mantine/core';
import { useCallback } from 'react';

import { sourcesOf, useBusStore } from '../../../stores/bus-store';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { DestinationTile } from './DestinationTile';

type DestinationTilesProps = {
  /**
   * The source these tiles route out of, or null when the channel has none
   * patched — there is nothing to route until it does.
   */
  deviceIdentifier: string | null;
};

/** Which destinations one source reaches. Click a tile to add or remove it. */
export const DestinationTiles = ({ deviceIdentifier }: DestinationTilesProps) => {
  const { outputs } = usePatchOutputs();
  const buses = useBusStore((state) => state.buses);
  const setOutputSources = useBusStore((state) => state.setOutputSources);

  // Written through the destination even though it is clicked on the source:
  // routing is stored as what each destination receives, so both sides of a
  // connection are the same edit.
  const toggle = useCallback(
    (deviceId: string) => {
      if (!deviceIdentifier) {
        return;
      }

      const current = sourcesOf(buses, deviceId);
      const next = current.includes(deviceIdentifier)
        ? current.filter((id) => id !== deviceIdentifier)
        : [...current, deviceIdentifier];

      void setOutputSources(deviceId, next);
    },
    [buses, deviceIdentifier, setOutputSources]
  );

  if (!deviceIdentifier || outputs.length === 0) {
    return null;
  }

  return (
    // Scrolls rather than clips: a named tile is wide enough that a handful of
    // destinations runs past the card, and a tile you cannot reach cannot be routed.
    <Group gap="3xs" wrap="nowrap" style={{ overflowX: 'auto', minWidth: 0 }}>
      {outputs.map((output, index) => (
        <DestinationTile
          key={output.id}
          deviceId={output.id}
          name={output.name}
          index={index}
          on={sourcesOf(buses, output.id).includes(deviceIdentifier)}
          onToggle={toggle}
        />
      ))}
    </Group>
  );
};
