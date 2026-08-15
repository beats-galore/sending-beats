import { Box } from '@mantine/core';

import { color, meterGradient } from '../../../theme/tokens';

import type { ColorToken } from '../../../theme/tokens';

type LevelMeterProps = {
  /** Normalised meter position, 0 to 1. */
  level: number;
  height?: number;
  /**
   * Surface the meter sits on. The segment overlay is drawn in this colour so
   * the ticks read as gaps cut out of the bar rather than as marks on top of it.
   */
  surface?: ColorToken;
  /** Dims the bar without hiding it, for muted sources. */
  dimmed?: boolean;
  /** What the bar reads as below the warning range. Defaults to the accent. */
  base?: string;
};

/** A horizontal signal meter. */
export const LevelMeter = ({
  level,
  height = 7,
  surface = 'panel',
  dimmed = false,
  base,
}: LevelMeterProps) => (
  <Box
    style={{
      height,
      background: color.bg,
      borderRadius: 'var(--mantine-radius-2xs)',
      position: 'relative',
      overflow: 'hidden',
    }}
  >
    <Box
      style={{
        position: 'absolute',
        left: 0,
        top: 0,
        bottom: 0,
        width: `${Math.min(100, Math.max(0, level * 100))}%`,
        background: meterGradient(90, base),
        opacity: dimmed ? 0.25 : 1,
        transition: 'width 90ms linear',
      }}
    />
    <Box
      style={{
        position: 'absolute',
        inset: 0,
        background: `repeating-linear-gradient(90deg, transparent 0 4px, ${color[surface]} 4px 6px)`,
      }}
    />
  </Box>
);
