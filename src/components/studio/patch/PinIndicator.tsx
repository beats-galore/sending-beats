import { Box } from '@mantine/core';

import type { PinEdge } from '../../../services/patch-layout-service';
import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { color } from '../../../theme/tokens';
import type { NodeRect } from './patch-layout';
import type { PatchRects } from './patch-rects';

const THICKNESS = 4;

type PinIndicatorProps = {
  rects: PatchRects;
};

/** The edge itself, as a bar lying along it. */
const edgeBar = (anchor: NodeRect, edge: PinEdge) => {
  switch (edge) {
    case 'bottom':
      return {
        left: anchor.left,
        top: anchor.top + anchor.height - THICKNESS / 2,
        width: anchor.width,
        height: THICKNESS,
      };
    case 'left':
      return {
        left: anchor.left - THICKNESS / 2,
        top: anchor.top,
        width: THICKNESS,
        height: anchor.height,
      };
    case 'right':
      return {
        left: anchor.left + anchor.width - THICKNESS / 2,
        top: anchor.top,
        width: THICKNESS,
        height: anchor.height,
      };
  }
};

/**
 * The edge a dragged node is about to pin to.
 *
 * Pinning happens by dropping a node flush against another, which is only a
 * deliberate act if you can see it coming — without this, a pin and a near miss
 * look identical until the pointer is released.
 */
export const PinIndicator = ({ rects }: PinIndicatorProps) => {
  const pinTarget = usePatchLayoutStore((state) => state.pinTarget);
  const anchor = pinTarget && rects.byKey[pinTarget.anchor];

  if (!pinTarget || !anchor) {
    return null;
  }

  return (
    <Box
      style={{
        position: 'absolute',
        ...edgeBar(anchor, pinTarget.edge),
        background: color.acc,
        borderRadius: THICKNESS,
        boxShadow: `0 0 12px ${color.acc}`,
        pointerEvents: 'none',
        zIndex: 30,
      }}
    />
  );
};
