import { Box } from '@mantine/core';

import type { MantineFontSize } from '@mantine/core';
import type { MouseEventHandler, ReactNode } from 'react';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

export const PillTone = ['accent', 'warn', 'hot', 'neutral', 'muted'] as const;
export type PillTone = (typeof PillTone)[number];

type PillProps = {
  children: ReactNode;
  tone?: PillTone;
  /** Solid fill instead of an outline — reads as engaged rather than available. */
  filled?: boolean;
  size?: MantineFontSize;
  onClick?: MouseEventHandler<HTMLDivElement>;
  title?: string;
};

type ToneSpec = { fg: string; bd: string; filledBg: string; filledFg: string };

const TONES: Record<PillTone, ToneSpec> = {
  accent: { fg: color.acc, bd: color.acc, filledBg: color.acc, filledFg: color.bg },
  warn: { fg: color.warn, bd: color.warn, filledBg: color.warn, filledFg: color.bg },
  hot: { fg: color.hotText, bd: color.hotBorder, filledBg: color.hot, filledFg: color.bg },
  neutral: { fg: color.textDim, bd: color.line, filledBg: color.panelHi, filledFg: color.text },
  muted: { fg: color.textFaint, bd: color.line, filledBg: color.dead, filledFg: color.textDim },
};

/**
 * The small state tag used throughout — role labels, ON/OFF flags, mute and
 * solo. Outlined means available, filled means engaged.
 */
export const Pill = ({
  children,
  tone = 'neutral',
  filled = false,
  size = '3xs',
  onClick,
  title,
}: PillProps) => {
  const spec = TONES[tone];

  return (
    <Box
      onClick={onClick}
      // A clickable pill in a node's title bar must not pick the node up instead
      data-no-drag={onClick ? '' : undefined}
      title={title}
      fz={size}
      style={{
        flex: 'none',
        padding: '2px 7px',
        borderRadius: 'var(--mantine-radius-xs)',
        fontWeight: 600,
        letterSpacing: layout.tracking.wide,
        whiteSpace: 'nowrap',
        border: `1px solid ${filled ? spec.filledBg : spec.bd}`,
        background: filled ? spec.filledBg : undefined,
        color: filled ? spec.filledFg : spec.fg,
        cursor: onClick ? 'pointer' : undefined,
      }}
    >
      {children}
    </Box>
  );
};
