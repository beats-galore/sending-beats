import { Box } from '@mantine/core';

import type { MouseEventHandler } from 'react';
import { color, glow } from '../../../theme/tokens';

export const StatusTone = ['accent', 'warn', 'hot', 'inert', 'dead'] as const;
export type StatusTone = (typeof StatusTone)[number];

type StatusDotProps = {
  tone: StatusTone;
  size?: number;
  /** Suppresses the bloom — used where a dot is a legend swatch, not a state. */
  flat?: boolean;
  onClick?: MouseEventHandler<HTMLDivElement>;
  title?: string;
};

const TONE_COLOR: Record<StatusTone, string> = {
  accent: color.acc,
  warn: color.warn,
  hot: color.hot,
  inert: color.textFaintest,
  dead: color.dead,
};

/** Signal indicator. Lit tones bloom; inert ones sit flat against the panel. */
export const StatusDot = ({ tone, size = 6, flat = false, onClick, title }: StatusDotProps) => {
  const lit = tone === 'accent' || tone === 'warn' || tone === 'hot';

  return (
    <Box
      onClick={onClick}
      title={title}
      style={{
        width: size,
        height: size,
        flex: 'none',
        borderRadius: '50%',
        background: TONE_COLOR[tone],
        boxShadow:
          lit && !flat
            ? glow(tone === 'hot' ? 'hot' : tone === 'warn' ? 'warn' : 'acc')
            : undefined,
        cursor: onClick ? 'pointer' : undefined,
      }}
    />
  );
};
