import { useCallback } from 'react';

import type { PointerEvent as ReactPointerEvent } from 'react';
import type { PatchTargetKey } from '../../../services/patch-color-service';
import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { clampToCanvas, snapToGrid } from '../patch/patch-layout';
import type { NodeRect } from '../patch/patch-layout';

/**
 * Controls inside a node's title bar that must not start a drag.
 *
 * The grip is the title bar, and the title bar is also where a channel is
 * renamed, a device is chosen and a node is deleted. Pressing on any of those
 * has to reach the control rather than pick the node up.
 */
const CONTROL_SELECTOR = 'input, select, textarea, button, [data-no-drag]';

/**
 * Makes a node's title bar a grip that moves it.
 *
 * The canvas is authored at a fixed width and scaled to fit, so pointer
 * movement is in screen pixels while positions are in canvas ones. The scale is
 * read back off the node being dragged — its width on screen against the width
 * it was drawn at — rather than threaded down from the view, so a node cannot
 * be handed a stale one.
 *
 * Nothing is written down until the pointer is released. Storing each frame
 * would put a few hundred round trips through a single drag.
 */
export const useNodeDrag = (targetKey: PatchTargetKey, rect: NodeRect) => {
  const place = usePatchLayoutStore((state) => state.place);
  const save = usePatchLayoutStore((state) => state.save);

  return useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      if (event.button !== 0 || (event.target as HTMLElement).closest(CONTROL_SELECTOR)) {
        return;
      }

      // A press that goes on to drag would otherwise start a text selection,
      // and sweeping the pointer across the canvas highlights every label it
      // passes over.
      event.preventDefault();

      const grip = event.currentTarget.getBoundingClientRect();
      const scale = grip.width > 0 ? grip.width / rect.width : 1;
      const origin = { x: event.clientX, y: event.clientY };
      let moved = false;

      const apply = (point: PointerEvent) => {
        const free = point.altKey;
        const left = rect.left + (point.clientX - origin.x) / scale;
        const top = rect.top + (point.clientY - origin.y) / scale;
        const snapped = {
          left: free ? left : snapToGrid(left),
          top: free ? top : snapToGrid(top),
        };
        const position = clampToCanvas({ ...rect, ...snapped });

        place(targetKey, { x: position.x, y: position.y });
      };

      const handleMove = (moveEvent: PointerEvent) => {
        moved = true;
        apply(moveEvent);
      };

      const handleUp = () => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);

        if (!moved) {
          return;
        }

        void save(targetKey);

        // A drag ends with a click on whatever ancestor the press and release
        // share, which for a node is the node itself — and every node selects
        // on click. Without this, moving a node would also focus it. The same
        // treatment `use-drag-value` gives the faders.
        const swallowDragClick = (clickEvent: MouseEvent) => clickEvent.stopPropagation();
        window.addEventListener('click', swallowDragClick, { capture: true, once: true });
        // Not every drag ends in a click; drop the listener rather than leave it
        // waiting to eat an unrelated one.
        setTimeout(() => window.removeEventListener('click', swallowDragClick, true), 0);
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
    },
    [targetKey, rect, place, save]
  );
};

/**
 * Whether this node is the one drawn above the rest, and how it gets there.
 *
 * Every node brings itself forward when it is pressed, anywhere on it. That is
 * what makes a stack of overlapping nodes something you can leaf through: the
 * one behind comes to the front when you reach for it, rather than the one in
 * front staying put because it happens to be drawn later.
 */
export const useNodeFront = (targetKey: PatchTargetKey) => {
  const front = usePatchLayoutStore((state) => state.front === targetKey);
  const bringToFront = usePatchLayoutStore((state) => state.bringToFront);

  return {
    front,
    bringToFront: useCallback(() => bringToFront(targetKey), [bringToFront, targetKey]),
  };
};
