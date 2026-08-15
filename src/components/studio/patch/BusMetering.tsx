import { Box, Group, Stack, Text } from '@mantine/core';

import type { StereoLevels } from '../../../stores/vu-meter-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { asGain, meterPosition } from '../format';
import { LevelColumn } from '../primitives/LevelColumn';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatRow } from '../primitives/StatRow';

const SCALE_MARKS = ['0', '-6', '-12', '-18', '-24', '-36', '-60'];

type BusMeteringProps = {
  levels: StereoLevels;
  gainDb: number;
};

/** What a mix gains when it is opened: the metering column, the readout and the stats. */
export const BusMetering = ({ levels, gainDb }: BusMeteringProps) => (
  <Group gap="3xl" pt="md" align="stretch" wrap="nowrap" style={{ flex: 1, minHeight: 0 }}>
    <Group gap="lg" align="stretch" wrap="nowrap">
      <LevelColumn level={meterPosition(levels.left.peak_level)} />
      <LevelColumn level={meterPosition(levels.right.peak_level)} />
      <Stack justify="space-between" gap={0} py="3xs">
        {SCALE_MARKS.map((mark) => (
          <Text key={mark} size="3xs" c={color.textFaintest}>
            {mark}
          </Text>
        ))}
      </Stack>
    </Group>

    <Stack gap="lg" style={{ flex: 1, minWidth: 0 }}>
      <Box
        p="md"
        style={{
          background: color.bg,
          border: border(),
          borderRadius: 'var(--mantine-radius-md)',
        }}
      >
        <Text fz="4xl" fw={600} c={color.acc} style={{ letterSpacing: layout.tracking.tight }}>
          {asGain(gainDb).replace('dB', '')}
          <Text span fz="lg" c={color.textFaint}>
            {' '}
            dB
          </Text>
        </Text>
        <SectionLabel tone="faint" tracking="widest" mt="3xs">
          MIX GAIN
        </SectionLabel>
      </Box>

      <Stack gap="xs">
        <StatRow label="PEAK L/R">
          <Text size="xs" c={color.textDim}>
            {levels.left.peak_level.toFixed(2)} / {levels.right.peak_level.toFixed(2)}
          </Text>
        </StatRow>
        <StatRow label="RMS L/R">
          <Text size="xs" c={color.textDim}>
            {levels.left.rms_level.toFixed(2)} / {levels.right.rms_level.toFixed(2)}
          </Text>
        </StatRow>
        {/* Loudness metering is not produced by the engine yet. */}
        <StatRow label="LUFS-S">—</StatRow>
      </Stack>
    </Stack>
  </Group>
);
