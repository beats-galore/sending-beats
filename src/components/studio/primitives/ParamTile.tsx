import { Box, Stack, Text } from '@mantine/core';

import { border, color } from '../../../theme/tokens';
import { DragBar } from './DragBar';
import { SectionLabel } from './SectionLabel';

type ParamTileProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  /** Decimal places used to format the reading. */
  precision?: number;
  unit?: string;
};

/** An inset well holding one named parameter and its scrub track. */
export const ParamTile = ({
  label,
  value,
  min,
  max,
  onChange,
  precision = 1,
  unit,
}: ParamTileProps) => (
  <Box
    style={{
      minWidth: 0,
      background: color.bg,
      border: border(),
      borderRadius: 'var(--mantine-radius-md)',
      padding: '8px 9px',
    }}
  >
    <Stack gap="xs">
      <SectionLabel tracking="wide" style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>
        {label}
      </SectionLabel>
      <Text fz="xl" fw={600} style={{ whiteSpace: 'nowrap' }}>
        {value.toFixed(precision)}
        {unit && (
          <Text span fz="2xs" c={color.textFaint}>
            {' '}
            {unit}
          </Text>
        )}
      </Text>
      <DragBar value={value} min={min} max={max} onChange={onChange} />
    </Stack>
  </Box>
);
