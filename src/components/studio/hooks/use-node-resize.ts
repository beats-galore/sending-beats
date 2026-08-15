import { useCallback } from 'react';

import type { PointerEvent as ReactPointerEvent } from 'react';
import type { PatchTargetKey } from '../../../services/patch-color-service';
import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { clampSize, snapToGrid } from '../patch/patch-layout';
import type { NodeRect, Size } from '../patch/patch-layout';

/**
 * The grip's own size in canvas coordinates.
 *
 * Also how the scale is recovered: the canvas is authored at a fixed width and
 * scaled to fit, so the grip's width on screen against the width it was drawn
 * at is exactly the factor pointer movement has to be divided by.
 */
export const RESIZE_GRIP_SIZE = 16;

/** Sets a node's size outright — what the condense and expand toggle does. */
export const useNodeSize = (targetKey: PatchTargetKey) => {
  const place = usePatchLayoutStore((state) => state.place);
  const save = usePatchLayoutStore((state) => state.save);

  return useCallback(
    (size: Size) => {
      place(targetKey, size);
      void save(targetKey);
    },
    [targetKey, place, save]
  );
};

/**
 * Makes the corner grip resize its node.
 *
 * Both axes, because how open a node is follows from how big it is — dragging a
 * source taller is the same gesture as opening it, and the toggle is only a
 * shortcut to the size it would have been dragged to.
 *
 * Sizes snap to the same dot grid positions do, and ⌥ bypasses it the same way.
 * Nothing is written until the pointer is released.
 */
export const useNodeResize = (targetKey: PatchTargetKey, rect: NodeRect, minimum: Size) => {
  const place = usePatchLayoutStore((state) => state.place);
  const save = usePatchLayoutStore((state) => state.save);
  const setMoving = usePatchLayoutStore((state) => state.setMoving);

  return useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      if (event.button !== 0) {
        return;
      }

      // The node itself selects on click and its title bar drags it. Neither
      // should happen because the corner was pressed.
      event.stopPropagation();

      setMoving(targetKey);
      const grip = event.currentTarget.getBoundingClientRect();
      const scale = grip.width > 0 ? grip.width / RESIZE_GRIP_SIZE : 1;
      const origin = { x: event.clientX, y: event.clientY };
      let moved = false;

      const apply = (point: PointerEvent) => {
        const free = point.altKey;
        const width = rect.width + (point.clientX - origin.x) / scale;
        const height = rect.height + (point.clientY - origin.y) / scale;
        const size = clampSize(
          {
            width: free ? width : snapToGrid(width),
            height: free ? height : snapToGrid(height),
          },
          rect.left,
          minimum
        );

        place(targetKey, size);
      };

      const handleMove = (moveEvent: PointerEvent) => {
        moved = true;
        apply(moveEvent);
      };

      const handleUp = () => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);
        setMoving(null);

        if (moved) {
          void save(targetKey);
        }
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
    },
    [targetKey, rect, minimum, place, save, setMoving]
  );
};
