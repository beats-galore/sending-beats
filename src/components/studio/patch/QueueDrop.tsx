import { Box, Text } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import { layout } from '../../../theme/layout';
import { color, dashedBorder } from '../../../theme/tokens';

type QueueDropProps = {
  tint: string;
  /** Whether files are currently over the panel this belongs to. */
  over: boolean;
  onBrowse: () => void;
};

/**
 * How files get into the queue.
 *
 * Two ways into one control: drop them on the panel, or click here and pick
 * them. Dragging is the quicker of the two when it works, and browsing is the
 * one that always does — from a window that is partly off screen, from a folder
 * that is awkward to reach, or with no second hand free.
 *
 * The panel takes the drop, not this. A strip this size is a small thing to
 * hit, and it is the first thing to go under the dock when the card it belongs
 * to sits low on the canvas.
 */
export const QueueDrop = ({ tint, over, onBrowse }: QueueDropProps) => {
  const { hovered, ref } = useHover();
  const lit = over || hovered;

  return (
    <Box
      ref={ref}
      onClick={(event) => {
        event.stopPropagation();
        onBrowse();
      }}
      title="Choose audio files to add to the queue"
      style={{
        flex: 'none',
        margin: '0 8px 8px',
        height: 44,
        borderRadius: 'var(--mantine-radius-sm)',
        display: 'grid',
        placeItems: 'center',
        cursor: 'pointer',
        border: lit ? `1px dashed ${tint}` : dashedBorder(),
        background: over ? color.panelHi : undefined,
      }}
    >
      <Text
        fz="3xs"
        c={lit ? tint : color.textFaint}
        style={{ letterSpacing: layout.tracking.caps }}
      >
        {over ? 'RELEASE TO ADD TO QUEUE' : 'BROWSE — OR DROP AUDIO FILES HERE'}
      </Text>
    </Box>
  );
};
