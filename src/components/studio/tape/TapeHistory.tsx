import { Group, Stack, Text } from '@mantine/core';
import { useHover } from '@mantine/hooks';
import { revealItemInDir } from '@tauri-apps/plugin-opener';

import { border, color } from '../../../theme/tokens';
import type { RecordingHistoryEntry } from '../../../types/audio.types';
import { asBytes, asClock } from '../format';
import { Panel } from '../primitives/Panel';
import { SectionLabel } from '../primitives/SectionLabel';
import { formatLabel } from './recording-format';

type TapeHistoryProps = {
  history: RecordingHistoryEntry[];
};

type TakeRowProps = {
  take: RecordingHistoryEntry;
};

const COLUMNS = [
  { label: 'NAME', width: undefined },
  { label: 'LENGTH', width: 90 },
  { label: 'SIZE', width: 80 },
  { label: 'FORMAT', width: 110 },
];

const TakeRow = ({ take }: TakeRowProps) => {
  const { hovered, ref } = useHover();

  return (
    <Group gap="xl" wrap="nowrap" py="lg" style={{ borderBottom: border() }}>
      <Text size="sm" truncate style={{ flex: 1, minWidth: 0 }}>
        {take.file_path.split('/').pop()}
      </Text>
      <Text size="sm" c={color.textDim} w={90}>
        {asClock(take.duration_seconds)}
      </Text>
      <Text size="sm" c={color.textDim} w={80}>
        {asBytes(take.file_size_bytes)}
      </Text>
      <Text size="sm" c={color.textDim} w={110}>
        {formatLabel(take.format)}
      </Text>
      <Text
        ref={ref}
        size="sm"
        w={60}
        ta="right"
        c={hovered ? color.acc : color.textFaint}
        style={{ cursor: 'pointer' }}
        onClick={() => void revealItemInDir(take.file_path)}
      >
        REVEAL
      </Text>
    </Group>
  );
};

/** Takes already on disk. */
export const TapeHistory = ({ history }: TapeHistoryProps) => (
  <Panel
    p="3xl"
    style={{ flex: 1, minHeight: 0 }}
    title={<SectionLabel tracking="widest">RECENT TAKES</SectionLabel>}
    action={
      <SectionLabel tone="faint" tracking="tight">
        {history.length} on disk
      </SectionLabel>
    }
  >
    <Stack gap={0}>
      <Group gap="xl" wrap="nowrap" pb="sm" style={{ borderBottom: border() }}>
        {COLUMNS.map((column) => (
          <SectionLabel
            key={column.label}
            tone="faint"
            tracking="caps"
            style={column.width ? { width: column.width } : { flex: 1 }}
          >
            {column.label}
          </SectionLabel>
        ))}
        <div style={{ width: 60 }} />
      </Group>

      {history.length === 0 ? (
        <Text size="xs" c={color.textFaint} ta="center" py="4xl">
          Nothing recorded yet.
        </Text>
      ) : (
        history.map((take) => <TakeRow key={take.id} take={take} />)
      )}
    </Stack>
  </Panel>
);
