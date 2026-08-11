import { Paper, Text } from '@mantine/core';

import type { MantineFontSize } from '@mantine/core';
import type { ReactNode } from 'react';
import { color } from '../../../theme/tokens';
import { SectionLabel } from './SectionLabel';


type StatTileProps = {
  label: ReactNode;
  value: ReactNode;
  /** Small trailing unit, set beside the value at label weight. */
  unit?: string;
  tone?: string;
  size?: MantineFontSize;
};

/** A headline reading with its name above it. Used for the at-a-glance metrics. */
export const StatTile = ({
  label,
  value,
  unit,
  tone = color.text,
  size = '5xl',
}: StatTileProps) => (
  <Paper p="2xl">
    <SectionLabel tracking="caps">{label}</SectionLabel>
    <Text fz={size} fw={600} c={tone} mt="xs" style={{ whiteSpace: 'nowrap' }}>
      {value}
      {unit && (
        <Text span fz="lg" c={color.textFaint}>
          {' '}
          {unit}
        </Text>
      )}
    </Text>
  </Paper>
);
