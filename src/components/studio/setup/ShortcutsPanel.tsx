import { SimpleGrid } from '@mantine/core';

import { Panel } from '../primitives/Panel';
import { StatRow } from '../primitives/StatRow';

const SHORTCUTS = [
  ['Mute focused channel', 'M'],
  ['Solo focused channel', 'S'],
  ['Start / stop the tape', '⌘R'],
  ['Connect / cut the stream', '⌘L'],
  ['Focus channel n', '⌥1…9'],
  ['Add a channel', '⌘N'],
] as const;

/** Keyboard shortcuts. */
export const ShortcutsPanel = () => (
  <Panel title="SHORTCUTS" p="3xl" style={{ flex: 1, minHeight: 0 }}>
    <SimpleGrid cols={2} spacing="5xl" verticalSpacing="sm">
      {SHORTCUTS.map(([action, key]) => (
        <StatRow key={action} label={action} size="sm">
          {key}
        </StatRow>
      ))}
    </SimpleGrid>
  </Panel>
);
