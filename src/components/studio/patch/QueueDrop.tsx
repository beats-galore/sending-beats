import { Box } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color, dashedBorder } from '../../../theme/tokens';
import type { FilePath } from '../../../types/util.types';
import { useFileDrop } from '../hooks/use-file-drop';

type QueueDropProps = {
  tint: string;
  onDrop: (paths: FilePath[]) => void;
};

/** Where files are dragged to build a queue. */
export const QueueDrop = ({ tint, onDrop }: QueueDropProps) => {
  const { ref, over } = useFileDrop(onDrop);

  return (
    <Box
      ref={ref}
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
};
