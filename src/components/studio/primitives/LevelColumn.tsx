import { Box } from '@mantine/core';

import { border, color, meterGradient } from '../../../theme/tokens';

import type { ColorToken } from '../../../theme/tokens';

type LevelColumnProps = {
  /** Normalised meter position, 0 to 1. */
  level: number;
  width?: number;
  height?: number | string;
  /** Surface behind the segment overlay — see `LevelMeter`. */
  surface?: ColorToken;
};

/** A vertical signal meter, filling from the bottom. */
export const LevelColumn = ({
  level,
  width = 20,
  height = '100%',
  surface = 'bgRaised',
}: LevelColumnProps) => (
  <Box
    style={{
      width,
      height,
      flex: 'none',
      background: color.bg,
      border: border(),
      borderRadius: 'var(--mantine-radius-xs)',
      position: 'relative',
      overflow: 'hidden',
    }}
  >
    <Box
      style={{
        position: 'absolute',
        left: 0,
        right: 0,
        bottom: 0,
        height: `${Math.min(100, Math.max(0, level * 100))}%`,
        background: meterGradient(0),
        transition: 'height 90ms linear',
      }}
    />
    <Box
      style={{
        position: 'absolute',
        inset: 0,
        background: `repeating-linear-gradient(0deg, transparent 0 4px, ${color[surface]} 4px 6px)`,
      }}
    />
  </Box>
);
