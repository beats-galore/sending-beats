import { Box, Group } from '@mantine/core';
import { useMemo } from 'react';

import { useCastConfigurationStore } from '../../../stores/cast-configuration-store';
import { transportLabel } from '../../../types/audio.types';
import { useAddSource } from '../hooks/use-add-source';
import type { SourceOption } from '../hooks/use-add-source';
import { useAvailableCastTargets } from '../hooks/use-cast-destination';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { SectionLabel } from '../primitives/SectionLabel';
import { AddTile } from './AddTile';
import { border, color } from '../../../theme/tokens';

/**
 * Where things are added to the patch.
 *
 * Pinned to the bottom of the viewport rather than sitting at the foot of each
 * column. Nodes can be dragged anywhere, so a target that lives in a column
 * ends up wherever that column drifted to — behind a node, off the bottom, or
 * somewhere you have to go looking. This does not move.
 *
 * One tile per kind rather than one button and a menu of everything: what you
 * want to add is a decision you have already made by the time you reach for it,
 * and it is what makes each list short enough to read.
 */
export const PatchDock = () => {
  const { physical, virtual, applications, players, add } = useAddSource();
  const { available, selectOutput } = usePatchOutputs();
  const castOptions = useAvailableCastTargets();
  const addCastTarget = useCastConfigurationStore((state) => state.addTarget);

  const destinations = useMemo(
    (): SourceOption[] =>
      available.map((device) => ({
        value: device.id,
        label: device.name,
        detail: transportLabel(device.transport),
      })),
    [available]
  );

  const stations = useMemo(
    (): SourceOption[] =>
      castOptions.map((station) => ({
        value: station.id,
        label: station.name,
        detail: 'broadcast',
      })),
    [castOptions]
  );

  return (
    <Box
      style={{
        flex: 'none',
        borderTop: border(),
        background: color.panelNav,
        padding: '10px 16px',
      }}
    >
      <Group gap="md" wrap="wrap">
        <SectionLabel>ADD</SectionLabel>

        <AddTile
          label="+ PHYSICAL INPUT"
          options={physical}
          emptyHint="Every input is patched"
          onPick={(value) => void add(value)}
        />
        <AddTile
          label="+ VIRTUAL INPUT"
          options={virtual}
          emptyHint="Every virtual device is patched"
          onPick={(value) => void add(value)}
        />
        <AddTile
          label="+ APPLICATION"
          options={applications}
          emptyHint="No applications are playing"
          onPick={(value) => void add(value)}
        />
        <AddTile
          label="+ QUEUE"
          options={players}
          emptyHint="No queues available"
          onPick={(value) => void add(value)}
        />

        <Box style={{ flex: 1 }} />

        {/* Its own button rather than another row in the destination list: a
            broadcast is not a piece of hardware, and it only offers itself
            while this patch is not already pointed at a station. */}
        {stations.length > 0 && (
          <AddTile
            label="+ CAST"
            options={stations}
            emptyHint="Every station is on this patch"
            onPick={(value) => void addCastTarget(value)}
          />
        )}

        <AddTile
          label="+ DESTINATION"
          options={destinations}
          emptyHint="Every output is patched"
          onPick={selectOutput}
        />
      </Group>
    </Box>
  );
};
