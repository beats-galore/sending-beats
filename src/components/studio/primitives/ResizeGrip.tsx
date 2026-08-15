import { Box } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import type { PointerEventHandler } from 'react';
import { color } from '../../../theme/tokens';
import { RESIZE_GRIP_SIZE } from '../hooks/use-node-resize';

type ResizeGripProps = {
  onResize: PointerEventHandler<HTMLDivElement>;
};

/**
 * The corner a node is resized by.
 *
 * Two hairlines rather than a filled tab: it sits on every node at once, and
 * the canvas is dense enough already. It takes the accent on hover, which is
 * where it explains itself.
 */
export const ResizeGrip = ({ onResize }: ResizeGripProps) => {
  const { hovered, ref } = useHover();
  const edge = `2px solid ${hovered ? color.acc : color.line}`;

  return (
    <Box
      ref={ref}
      onPointerDown={onResize}
      // Sits inside the node, which selects on click and drags by its title bar
      data-no-drag
      title="Drag to resize — hold ⌥ to place freely"
      style={{
        position: 'absolute',
        right: 2,
        bottom: 2,
        width: RESIZE_GRIP_SIZE,
        height: RESIZE_GRIP_SIZE,
        cursor: 'nwse-resize',
        borderRight: edge,
        borderBottom: edge,
        borderBottomRightRadius: 'var(--mantine-radius-lg)',
      }}
    />
  );
};
