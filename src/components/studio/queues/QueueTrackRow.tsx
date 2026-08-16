import { Box, Group, Stack, Text } from '@mantine/core';

import type { PointerEvent as ReactPointerEvent } from 'react';
import { border, color } from '../../../theme/tokens';
import type { QueueTrack } from '../../../types/file-player.types';
import { queueTrackTitle } from '../../../types/file-player.types';
import { asTrackTime } from '../format';
import { reorderRow } from '../hooks/use-drag-reorder';
import { DeleteButton } from '../primitives/DeleteButton';

type QueueTrackRowProps = {
  track: QueueTrack;
  /** Where it sits now, which is the number shown. */
  index: number;
  /** Whether this is the row being dragged. */
  dragging: boolean;
  onGrab: (event: ReactPointerEvent<HTMLElement>) => void;
  onRemove: () => void;
};

/** One track in a queue, on the screen where the queue is edited. */
export const QueueTrackRow = ({
  track,
  index,
  dragging,
  onGrab,
  onRemove,
}: QueueTrackRowProps) => (
  <Group
    {...reorderRow}
    gap="md"
    wrap="nowrap"
    px="sm"
    py="xs"
    style={{
      borderRadius: 'var(--mantine-radius-sm)',
      border: dragging ? border('acc') : '1px solid transparent',
      background: dragging ? color.panelHi : undefined,
      // Lifted while dragging so it reads as the thing being moved rather than
      // as a row that happens to be highlighted.
      boxShadow: dragging ? 'var(--mantine-shadow-md)' : undefined,
    }}
  >
    {/* The grip, not the whole row: a row you cannot click without dragging is
        a row you cannot select, and this list will grow ways to act on one. */}
    <Box
      onPointerDown={onGrab}
      title="Drag to reorder"
      style={{
        flex: 'none',
        cursor: 'grab',
        color: color.textFaintest,
        lineHeight: 1,
        fontSize: 12,
        touchAction: 'none',
        userSelect: 'none',
      }}
    >
      ⠿
    </Box>

    <Text size="2xs" c={color.textFaintest} w={20} style={{ flex: 'none' }}>
      {String(index + 1).padStart(2, '0')}
    </Text>

    <Stack gap={0} style={{ flex: 1, minWidth: 0 }}>
      <Text size="sm" truncate>
        {queueTrackTitle(track)}
      </Text>
      <Text size="2xs" c={color.textFaintest} truncate>
        {track.artist ?? 'unknown artist'}
      </Text>
    </Stack>

    <Text size="2xs" c={color.textFaint} style={{ flex: 'none' }}>
      {track.durationMs === null ? '--:--' : asTrackTime(track.durationMs / 1000)}
    </Text>

    <DeleteButton onDelete={onRemove} title={`Remove ${queueTrackTitle(track)} from this queue`} />
  </Group>
);
