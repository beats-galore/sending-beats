import { Box } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import type { PatchTargetKey } from '../../../services/patch-color-service';
import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { color } from '../../../theme/tokens';
import type { NodeRect } from './patch-layout';
import type { Pin } from './patch-pins';

const SIZE = 18;

type PinSeamProps = {
  targetKey: PatchTargetKey;
  pin: Pin;
  /** Where the pinned node ended up, which is what the seam sits on. */
  rect: NodeRect;
};

/**
 * The joint between a pinned node and its anchor, and how it is undone.
 *
 * Sits on the seam rather than in either node's title bar, because a pin
 * belongs to neither node on its own — it is the thing between them, and that
 * is where you would reach to take it apart.
 */
export const PinSeam = ({ targetKey, pin, rect }: PinSeamProps) => {
  const unpin = usePatchLayoutStore((state) => state.unpin);
  const { hovered, ref } = useHover();

  // Centred on the edge the node is held against.
  const position =
    pin.edge === 'bottom'
      ? { left: rect.left + rect.width / 2 - SIZE / 2, top: rect.top - SIZE / 2 }
      : pin.edge === 'left'
        ? { left: rect.left + rect.width - SIZE / 2, top: rect.top + rect.height / 2 - SIZE / 2 }
        : { left: rect.left - SIZE / 2, top: rect.top + rect.height / 2 - SIZE / 2 };

  return (
    <Box
      ref={ref}
      onClick={(event) => {
        event.stopPropagation();
        // Released where it is being drawn, so letting go of a pin never moves
        // anything — which is what makes it safe to try.
        void unpin(targetKey, { x: rect.left, y: rect.top });
      }}
      // Sits over two nodes, either of which would otherwise take the press
      onPointerDown={(event) => event.stopPropagation()}
      data-no-drag
      title="Pinned — click to release"
      style={{
        position: 'absolute',
        ...position,
        width: SIZE,
        height: SIZE,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        borderRadius: '50%',
        background: color.bgRaised,
        border: `1px solid ${hovered ? color.hot : color.acc}`,
        color: hovered ? color.hotText : color.acc,
        cursor: 'pointer',
        fontSize: 9,
        lineHeight: 1,
        zIndex: 25,
      }}
    >
      {hovered ? '×' : '⚲'}
    </Box>
  );
};
