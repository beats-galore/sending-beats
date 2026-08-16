import { Box } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

type QueueBreakSeamProps = {
  /** Position of the track above this seam, for the label when a break is set. */
  index: number;
  /** Whether the break falls here. */
  broken: boolean;
  /** The player's colour, which a set break reads in. */
  tint: string;
  onToggle: () => void;
};

/**
 * The gap between two tracks, and what a break is set on.
 *
 * A break belongs between two tracks rather than to either of them, so the gap
 * is the control: the place you want the player to stop is the place you click.
 */
export const QueueBreakSeam = ({ index, broken, tint, onToggle }: QueueBreakSeamProps) => (
  <Box
    onClick={(event) => {
      event.stopPropagation();
      onToggle();
    }}
    title={broken ? 'Play straight through here' : 'Pause after this track'}
    style={{
      flex: 'none',
      height: 14,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      position: 'relative',
      cursor: 'pointer',
    }}
  >
    <Box
      style={{
        position: 'absolute',
        left: 7,
        right: 7,
        height: broken ? 2 : 1,
        borderRadius: 1,
        background: broken ? tint : color.line,
        opacity: broken ? 1 : 0.45,
      }}
    />
    {broken && (
      <Box
        style={{
          position: 'relative',
          fontSize: 8,
          letterSpacing: layout.tracking.wider,
          whiteSpace: 'nowrap',
          padding: '1px 6px',
          borderRadius: 'var(--mantine-radius-2xs)',
          color: tint,
          border: `1px solid ${tint}`,
          background: color.bg,
        }}
      >
        ❙❙ PAUSE AFTER {String(index + 1).padStart(2, '0')}
      </Box>
    )}
  </Box>
);
