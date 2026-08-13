import { Group, Stack, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import type { LogEntry } from '../hooks/use-cast-telemetry';
import { Panel } from '../primitives/Panel';
import { SectionLabel } from '../primitives/SectionLabel';

type ConnectionLogProps = {
  entries: LogEntry[];
};

const TAG_COLOR: Record<LogEntry['tag'], string> = {
  OK: color.acc,
  WARN: color.warn,
  INFO: color.textFaint,
};

/** What the connection has done this session. */
export const ConnectionLog = ({ entries }: ConnectionLogProps) => (
  <Panel
    title={<SectionLabel tracking="widest">CONNECTION LOG</SectionLabel>}
    p="3xl"
    style={{ flex: 1, minHeight: 0 }}
  >
    <Stack gap="xs">
      {entries.length === 0 ? (
        <Text size="xs" c={color.textFaint}>
          Nothing logged yet this session.
        </Text>
      ) : (
        [...entries].reverse().map((entry) => (
          <Group key={entry.id} gap="lg" wrap="nowrap" align="flex-start">
            <Text size="xs" c={color.textFaintest} w={64} style={{ flex: 'none' }}>
              {entry.at}
            </Text>
            <Text size="xs" c={TAG_COLOR[entry.tag]} w={52} style={{ flex: 'none' }}>
              {entry.tag}
            </Text>
            <Text size="xs" c={color.textDim} style={{ flex: 1, minWidth: 0 }}>
              {entry.message}
            </Text>
          </Group>
        ))
      )}
    </Stack>
  </Panel>
);
