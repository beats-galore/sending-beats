import { Group, Stack } from '@mantine/core';

import { useTapeTransport } from '../hooks/use-tape-transport';
import { TapeHistory } from './TapeHistory';
import { TapeOutputSettings } from './TapeOutputSettings';
import { TapeTags } from './TapeTags';
import { TapeTransport } from './TapeTransport';

/** Recording: the transport, where takes land, their tags, and what is on disk. */
export const TapeView = () => {
  const tape = useTapeTransport();

  return (
    <Group
      align="stretch"
      gap="4xl"
      p="5xl"
      wrap="nowrap"
      style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}
    >
      <Stack w={420} gap="2xl" style={{ flex: 'none' }}>
        <TapeTransport tape={tape} />
        <TapeOutputSettings tape={tape} />
      </Stack>

      <Stack gap="2xl" style={{ flex: 1, minWidth: 0 }}>
        <TapeTags tape={tape} />
        <TapeHistory history={tape.history} />
      </Stack>
    </Group>
  );
};
