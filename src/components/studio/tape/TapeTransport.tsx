import { Box, Group, SimpleGrid, Stack, Text } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { asBytes, asClock, meterPosition } from '../format';
import type { useTapeTransport } from '../hooks/use-tape-transport';
import { LevelColumn } from '../primitives/LevelColumn';
import { Panel } from '../primitives/Panel';
import { PanelHeading } from '../primitives/PanelHeading';
import { Pill } from '../primitives/Pill';
import { StatRow } from '../primitives/StatRow';
import { formatLabel } from './recording-format';


type TapeTransportProps = {
  tape: ReturnType<typeof useTapeTransport>;
};

/** The recorder itself: arm it, watch it run, see what it has captured. */
export const TapeTransport = ({ tape }: TapeTransportProps) => {
  const levels = tape.status?.current_session?.current_levels ?? [0, 0];
  const fileName = tape.filePath?.split('/').pop();

  return (
    <Panel
      p="4xl"
      gap="3xl"
      title={
        <Group gap="md" wrap="nowrap" style={{ flex: 1 }}>
          <PanelHeading order={2}>THE TAPE</PanelHeading>
        </Group>
      }
      action={
        <Pill tone={tape.isRecording ? 'hot' : 'neutral'} filled={tape.isRecording} size="2xs">
          {tape.isRecording ? 'ROLLING' : 'READY'}
        </Pill>
      }
    >
      <Group gap="3xl" wrap="nowrap">
        <Box
          onClick={() => void tape.toggle()}
          style={{
            width: 78,
            height: 78,
            flex: 'none',
            borderRadius: '50%',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            background: tape.isRecording ? color.hot : 'transparent',
            border: `2px solid ${tape.isRecording ? color.hot : color.line}`,
          }}
        >
          <Box
            style={
              tape.isRecording
                ? {
                    width: 22,
                    height: 22,
                    background: color.bg,
                    borderRadius: 'var(--mantine-radius-xs)',
                  }
                : { width: 26, height: 26, borderRadius: '50%', background: color.hot }
            }
          />
        </Box>

        <Stack gap="2xs" style={{ flex: 1, minWidth: 0 }}>
          <Text fz="6xl" fw={600} style={{ letterSpacing: layout.tracking.tight }}>
            {asClock(tape.elapsedSeconds)}
          </Text>
          <Text size="xs" c={color.textDim} truncate>
            {tape.isRecording && fileName ? fileName : 'ready — nothing on tape yet'}
          </Text>
        </Stack>
      </Group>

      <Group gap="lg" align="stretch" wrap="nowrap">
        <LevelColumn level={meterPosition(levels[0])} height={52} surface="panel" />
        <LevelColumn level={meterPosition(levels[1])} height={52} surface="panel" />
        <SimpleGrid cols={2} spacing="4xl" verticalSpacing="xs" style={{ flex: 1 }}>
          <StatRow label="SIZE">{asBytes(tape.fileSizeBytes)}</StatRow>
          <StatRow label="FREE">{`${tape.availableSpaceGb.toFixed(0)} GB`}</StatRow>
          <StatRow label="FORMAT">{formatLabel(tape.config?.format)}</StatRow>
          <StatRow label="TOTAL">{`${tape.totalRecordings} takes`}</StatRow>
        </SimpleGrid>
      </Group>
    </Panel>
  );
};
