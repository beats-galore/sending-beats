import { Box } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color, dashedBorder } from '../../../theme/tokens';

type QueueDropProps = {
  tint: string;
  /** Whether files are currently over the panel this belongs to. */
  over: boolean;
};

/**
 * Where files are dragged to build a queue.
 *
 * The affordance, not the target — the whole panel accepts a drop. A 44-row
 * strip at the bottom of a panel is a small thing to hit, and it is the first
 * thing to go under the dock when the card it belongs to sits low on the
 * canvas, which left files being dropped at something that could not take them.
 */
export const QueueDrop = ({ tint, over }: QueueDropProps) => (
  <Box
    fz="3xs"
    style={{
      flex: 'none',
      margin: '0 8px 8px',
      height: 44,
      borderRadius: 'var(--mantine-radius-sm)',
      display: 'grid',
      placeItems: 'center',
      letterSpacing: layout.tracking.caps,
      border: over ? `1px dashed ${tint}` : dashedBorder(),
      background: over ? color.panelHi : undefined,
      color: over ? tint : color.textFaint,
    }}
  >
    {over ? 'RELEASE TO ADD TO QUEUE' : 'DROP AUDIO FILES HERE'}
  </Box>
);
