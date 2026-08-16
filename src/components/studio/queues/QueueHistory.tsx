import { Group, ScrollArea, Stack, Text } from '@mantine/core';

import { selectedQueue, useQueueStore } from '../../../stores/queue-store';
import { color } from '../../../theme/tokens';
import { queueTrackTitle } from '../../../types/file-player.types';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';

/**
 * When something played, as a listener would say it.
 *
 * The date only appears when it is not today: a log read during a show is all
 * from the last few hours, and stamping every row with the date would bury the
 * one part that is actually changing.
 */
const playedAtLabel = (played: string): string => {
  const at = new Date(played);
  const time = at.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  const isToday = at.toDateString() === new Date().toDateString();

  return isToday ? time : `${at.toLocaleDateString(undefined, { day: 'numeric', month: 'short' })} ${time}`;
};

/**
 * What this queue has played.
 *
 * A log rather than what is missing from the list: a track played three times
 * reads as three rows, and the list it came from is untouched.
 */
export const QueueHistory = () => {
  const selected = useQueueStore(selectedQueue);
  const plays = useQueueStore((state) => state.plays);
  const clearPlays = useQueueStore((state) => state.clearPlays);

  if (!selected) {
    return null;
  }

  return (
    <Panel
      title="PLAY HISTORY"
      action={
        plays.length > 0 && (
          <ActionButton tone="danger" onClick={() => void clearPlays(selected.id)}>
            CLEAR
          </ActionButton>
        )
      }
      style={{ flex: 1, minHeight: 0 }}
    >
      {plays.length === 0 ? (
        <Text size="xs" c={color.textFaint} ta="center" py="xl">
          Nothing has played from this queue yet.
        </Text>
      ) : (
        <ScrollArea style={{ flex: 1, minHeight: 0 }}>
          <Stack gap="3xs">
            {plays.map((play) => (
              <Group key={play.id} gap="md" wrap="nowrap" px="sm" py="xs">
                <Text size="2xs" c={color.textFaint} w={64} style={{ flex: 'none' }}>
                  {playedAtLabel(play.playedAt)}
                </Text>
                <Stack gap={0} style={{ flex: 1, minWidth: 0 }}>
                  <Text size="sm" truncate>
                    {queueTrackTitle(play)}
                  </Text>
                  <Text size="2xs" c={color.textFaintest} truncate>
                    {play.artist ?? 'unknown artist'}
                  </Text>
                </Stack>
                {/* A play whose track has since left the queue still reads —
                    that is why the log carries its own copy of the details. */}
                {play.trackId === null && (
                  <Text size="3xs" c={color.textFaintest} style={{ flex: 'none' }}>
                    removed
                  </Text>
                )}
              </Group>
            ))}
          </Stack>
        </ScrollArea>
      )}
    </Panel>
  );
};
