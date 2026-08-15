import { Box, Group, Stack, Text } from '@mantine/core';

import type { StereoLevels } from '../../../stores/vu-meter-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { asGain, meterPosition } from '../format';
import { LevelColumn } from '../primitives/LevelColumn';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatRow } from '../primitives/StatRow';

const COLUMN_HEIGHT = 64;

type BusMeteringProps = {
  levels: StereoLevels;
  gainDb: number;
};

/**
 * What a mix gains when it is opened: how loud it is, and how loud it has been.
 *
 * The readout sits beside the meters rather than in a panel of its own — the
 * number and the columns are answering the same question, and a card that is
 * mostly member rows has no room to say it twice.
 */
export const BusMetering = ({ levels, gainDb }: BusMeteringProps) => (
  <Stack gap="lg" style={{ flex: 'none' }}>
    <Group gap="lg" wrap="nowrap" align="center">
      <Group gap="2xs" wrap="nowrap" style={{ flex: 'none' }}>
        <LevelColumn level={meterPosition(levels.left.peak_level)} width={10} height={COLUMN_HEIGHT} />
        <LevelColumn
          level={meterPosition(levels.right.peak_level)}
          width={10}
          height={COLUMN_HEIGHT}
        />
      </Group>

      <Text fz="4xl" fw={600} c={color.acc} style={{ letterSpacing: layout.tracking.tight }}>
        {asGain(gainDb).replace('dB', '')}
        <Text span fz="lg" c={color.textFaint}>
          {' '}
          dB
        </Text>
      </Text>

      <SectionLabel tone="faint" tracking="widest" style={{ flex: 1, textAlign: 'right' }}>
        MIX GAIN
      </SectionLabel>
    </Group>

    <Box pt="md" style={{ borderTop: border() }}>
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
    </Box>
  </Stack>
);
