import { Group, ScrollArea, Stack, Text } from '@mantine/core';

import { selectedQueue, useQueueStore } from '../../../stores/queue-store';
import type { QueueTrack } from '../../../types/file-player.types';
import { color } from '../../../theme/tokens';
import { reorderList, useDragReorder, withDragApplied } from '../hooks/use-drag-reorder';
import { asTrackTime } from '../format';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';
import { QueueTrackRow } from './QueueTrackRow';

/** How long everything in the list runs, ignoring files that declare no length. */
const totalOf = (durations: (number | null)[]): number =>
  durations.reduce((sum: number, ms) => sum + (ms ?? 0), 0);

/** What is in the selected queue, in the order it plays. */
export const QueueContents = () => {
  const selected = useQueueStore(selectedQueue);
  const selectedId = selected?.id ?? null;
  const tracks = useQueueStore((state) => state.tracks);
  const targetIds = useQueueStore((state) => state.targetIds);
  const browse = useQueueStore((state) => state.browseForTracks);
  const addTarget = useQueueStore((state) => state.addTarget);
  const removeTarget = useQueueStore((state) => state.removeTarget);
  const moveTrack = useQueueStore((state) => state.moveTrack);
  const removeTrack = useQueueStore((state) => state.removeTrack);

  // Hooks before the early return: a queue not being picked yet is a thing to
  // draw, not a reason to have a different set of hooks.
  const { drag, start } = useDragReorder<QueueTrack['id']>({
    onMove: (trackId, toIndex) => {
      if (selectedId) {
        void moveTrack(selectedId, trackId, toIndex);
      }
    },
  });

  if (!selected) {
    return (
      <Panel style={{ flex: 1, minHeight: 0 }}>
        <Text size="xs" c={color.textFaint} ta="center" py="xl">
          Pick a queue to see what is in it.
        </Text>
      </Panel>
    );
  }

  const onPatch = targetIds.includes(selected.id);
  const total = totalOf(tracks.map((track) => track.durationMs));
  const shown = withDragApplied(tracks, drag);

  return (
    <Panel
      title={`${tracks.length} ${tracks.length === 1 ? 'TRACK' : 'TRACKS'} · ${asTrackTime(total / 1000)}`}
      action={
        <Group gap="sm" wrap="nowrap">
          {/* Putting a queue on the patch is what makes it available to a
              channel. Kept here rather than only in the dock, because this is
              where you are while deciding the queue is ready to go out. */}
          <ActionButton
            onClick={() =>
              onPatch ? void removeTarget(selected.id) : void addTarget(selected.id)
            }
          >
            {onPatch ? 'REMOVE FROM PATCH' : 'ADD TO PATCH'}
          </ActionButton>
          <ActionButton tone="accent" onClick={() => void browse(selected.id)}>
            + FILES
          </ActionButton>
        </Group>
      }
      style={{ flex: 1, minHeight: 0 }}
    >
      {tracks.length === 0 ? (
        <Text size="xs" c={color.textFaint} ta="center" py="xl">
          Nothing in this queue yet.
        </Text>
      ) : (
        <ScrollArea style={{ flex: 1, minHeight: 0 }}>
          {/* Drawn in the order the drag would leave it, so the rows move under
              the pointer rather than the dragged one merely lighting up. */}
          <Stack gap="3xs" {...reorderList}>
            {shown.map((track, index) => (
              <QueueTrackRow
                key={track.id}
                track={track}
                index={index}
                dragging={drag?.id === track.id}
                onGrab={(event) => start(event, track.id, index, shown.length)}
                onRemove={() => selectedId && void removeTrack(selectedId, track.id)}
              />
            ))}
          </Stack>
        </ScrollArea>
      )}
    </Panel>
  );
};
