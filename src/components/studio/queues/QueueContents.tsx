import { Group, ScrollArea, Stack, Text } from '@mantine/core';

import { selectedQueue, useQueueStore } from '../../../stores/queue-store';
import { color } from '../../../theme/tokens';
import { queueTrackTitle } from '../../../types/file-player.types';
import { asTrackTime } from '../format';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';

/** How long everything in the list runs, ignoring files that declare no length. */
const totalOf = (durations: (number | null)[]): number =>
  durations.reduce((sum: number, ms) => sum + (ms ?? 0), 0);

/** What is in the selected queue, in the order it plays. */
export const QueueContents = () => {
  const selected = useQueueStore(selectedQueue);
  const tracks = useQueueStore((state) => state.tracks);
  const targetIds = useQueueStore((state) => state.targetIds);
  const browse = useQueueStore((state) => state.browseForTracks);
  const addTarget = useQueueStore((state) => state.addTarget);
  const removeTarget = useQueueStore((state) => state.removeTarget);

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
          <Stack gap="3xs">
            {tracks.map((track, index) => (
              <Group key={track.id} gap="md" wrap="nowrap" px="sm" py="xs">
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
              </Group>
            ))}
          </Stack>
        </ScrollArea>
      )}
    </Panel>
  );
};
