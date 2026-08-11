import { Box } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

import type { StatusTone } from './StatusDot';

type PortDotProps = {
  tone: StatusTone;
  /** Which edge of the parent panel the port straddles. */
  side: 'left' | 'right';
  /** Distance from the parent's top edge to the port's centre. */
  top: number;
};

const TONE_COLOR: Record<StatusTone, string> = {
  accent: color.acc,
  warn: color.warn,
  hot: color.hot,
  inert: color.textFaintest,
  dead: color.dead,
};

/** A patch point on the edge of a node, where a cable lands. */
export const PortDot = ({ tone, side, top }: PortDotProps) => (
  <Box
    style={{
      position: 'absolute',
      [side]: layout.patch.portInset,
      top: top - layout.patch.portSize / 2,
      width: layout.patch.portSize,
      height: layout.patch.portSize,
      borderRadius: '50%',
      background: color.bg,
      border: `2px solid ${TONE_COLOR[tone]}`,
    }}
  />
);
