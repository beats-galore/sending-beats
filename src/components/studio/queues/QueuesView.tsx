import { Group, Stack } from '@mantine/core';
import { useEffect } from 'react';

import { useQueueStore } from '../../../stores/queue-store';
import { QueueContents } from './QueueContents';
import { QueueHistory } from './QueueHistory';
import { QueuePicker } from './QueuePicker';

/**
 * Queues: the lists this studio plays from, and what they have played.
 *
 * A queue belongs here rather than to a patch — the same run of ads is the
 * station's, not whichever canvas happened to be open when it was built. A
 * patch points at one from the dock, and this is where it is made, filled and
 * read back.
 */
export const QueuesView = () => {
  const load = useQueueStore((state) => state.load);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <Group
      align="stretch"
      gap="4xl"
      p="5xl"
      wrap="nowrap"
      style={{ flex: 1, minHeight: 0, overflow: 'hidden' }}
    >
      <Stack w={360} gap="2xl" style={{ flex: 'none', minHeight: 0 }}>
        <QueuePicker />
      </Stack>

      {/* What is in it beside what it has done with it: the two questions you
          have about a queue are what it will play and what it played. */}
      <Group align="stretch" gap="2xl" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
        <QueueContents />
        <QueueHistory />
      </Group>
    </Group>
  );
};
