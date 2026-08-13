import { Box } from '@mantine/core';
import type { MantineFontSize } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import type { MouseEventHandler, ReactNode } from 'react';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

export const ActionTone = ['accent', 'ghost', 'hot', 'danger'] as const;
export type ActionTone = (typeof ActionTone)[number];

type ActionButtonProps = {
  children: ReactNode;
  onClick?: MouseEventHandler<HTMLDivElement>;
  tone?: ActionTone;
  size?: MantineFontSize;
  /** Stretches the button across its container and centres the label. */
  fullWidth?: boolean;
  padding?: string;
  disabled?: boolean;
};

type ToneSpec = {
  bg?: string;
  fg: string;
  bd?: string;
  hoverFg: string;
  hoverBd?: string;
};

const TONES: Record<ActionTone, ToneSpec> = {
  accent: { bg: color.acc, fg: color.bg, hoverFg: color.bg },
  ghost: { fg: color.textDim, bd: color.line, hoverFg: color.acc, hoverBd: color.acc },
  hot: {
    bg: color.hotBg,
    fg: color.hotText,
    bd: color.hotBorder,
    hoverFg: color.hot,
    hoverBd: color.hot,
  },
  danger: { fg: color.textDim, bd: color.line, hoverFg: color.hotText, hoverBd: color.hot },
};

/**
 * The studio's button. Ghost tones brighten to the accent on hover, which is how
 * the interface signals that a bordered label is actually pressable.
 */
export const ActionButton = ({
  children,
  onClick,
  tone = 'ghost',
  size = '2xs',
  fullWidth = false,
  padding = '7px 12px',
  disabled = false,
}: ActionButtonProps) => {
  const { hovered, ref } = useHover();
  const spec = TONES[tone];
  const active = hovered && !disabled;

  return (
    <Box
      ref={ref}
      onClick={disabled ? undefined : onClick}
      fz={size}
      style={{
        flex: fullWidth ? 1 : 'none',
        padding,
        textAlign: 'center',
        borderRadius: 'var(--mantine-radius-sm)',
        fontWeight: 600,
        letterSpacing: layout.tracking.wide,
        whiteSpace: 'nowrap',
        background: spec.bg,
        color: active ? spec.hoverFg : spec.fg,
        border: spec.bd ? `1px solid ${active ? (spec.hoverBd ?? spec.bd) : spec.bd}` : undefined,
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.5 : 1,
      }}
    >
      {children}
    </Box>
  );
};
