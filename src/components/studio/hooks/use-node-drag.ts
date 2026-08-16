import { useCallback } from 'react';

import type { PointerEvent as ReactPointerEvent } from 'react';
import type { PatchTargetKey } from '../../../services/patch-color-service';
import { PinEdge } from '../../../services/patch-layout-service';
import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { clampToCanvas, snapToGrid } from '../patch/patch-layout';
import type { NodeRect } from '../patch/patch-layout';
import { pinnedAt, pinnedGroup, pinOf } from '../patch/patch-pins';
import type { Pin } from '../patch/patch-pins';
import type { PatchRects } from '../patch/patch-rects';
import { usePatchRectsContext } from '../patch/patch-rects-context';

/**
 * Controls inside a node's title bar that must not start a drag.
 *
 * The grip is the title bar, and the title bar is also where a channel is
 * renamed, a device is chosen and a node is deleted. Pressing on any of those
 * has to reach the control rather than pick the node up.
 */
const CONTROL_SELECTOR = 'input, select, textarea, button, [data-no-drag]';

/** How close an edge has to come before the node snaps flush against it. */
const PIN_SNAP = 22;

/**
 * The edge this node is close enough to be dropped against, if any.
 *
 * Its own group is skipped: pinning a node to one of its own followers would
 * make a loop, and the arrangement would have no anchor to derive from.
 */
const pinCandidate = (
  targetKey: PatchTargetKey,
  dragged: NodeRect,
  rects: PatchRects,
  placements: Parameters<typeof pinnedGroup>[2]
): { pin: Pin; at: { left: number; top: number } } | null => {
  const own = new Set(pinnedGroup(targetKey, rects.keys, placements));

  for (const anchorKey of rects.keys) {
    if (own.has(anchorKey)) {
      continue;
    }

    const anchor = rects.byKey[anchorKey];
    for (const edge of PinEdge) {
      // Always against the first slot: a second node wanting the same edge is
      // dropped under the one already there, which pins it to that node
      // instead and builds a chain.
      const at = pinnedAt(anchor, edge, dragged, 0);
      if (
        Math.abs(at.left - dragged.left) <= PIN_SNAP &&
        Math.abs(at.top - dragged.top) <= PIN_SNAP
      ) {
        return { pin: { anchor: anchorKey, edge }, at };
      }
    }
  }

  return null;
};

/**
 * Makes a node's title bar a grip that moves it.
 *
 * The canvas is authored at a fixed width and scaled to fit, so pointer
 * movement is in screen pixels while positions are in canvas ones. The scale is
 * read back off the node being dragged — its width on screen against the width
 * it was drawn at — rather than threaded down from the view, so a node cannot
 * be handed a stale one.
 *
 * Dragging a node flush against another's edge pins the two together. ⌥ turns
 * that off along with the grid snap, which is the one rule the canvas has about
 * modifiers: ⌥ means no help, put it exactly where I said.
 *
 * Dragging a node that is already pinned pulls it out of its group — the pin is
 * released at the position it was being drawn at, so it comes away from under
 * the pointer rather than jumping.
 *
 * Nothing is written down until the pointer is released. Storing each frame
 * would put a few hundred round trips through a single drag.
 */
export const useNodeDrag = (targetKey: PatchTargetKey, rect: NodeRect) => {
  const place = usePatchLayoutStore((state) => state.place);
  const save = usePatchLayoutStore((state) => state.save);
  const setPinTarget = usePatchLayoutStore((state) => state.setPinTarget);
  const pin = usePatchLayoutStore((state) => state.pin);
  const rects = usePatchRectsContext();

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
      let landing: Pin | null = null;

      // A node being pulled out of a group was taking its size from its anchor.
      // Freezing that size as it comes away is what stops it springing back to
      // whatever it was before it was pinned.
      const detaching =
        pinOf(usePatchLayoutStore.getState().placements[targetKey], targetKey) !== null;
      const kept = detaching ? { width: rect.width, height: rect.height } : {};

      const apply = (point: PointerEvent) => {
        const free = point.altKey;
        const left = rect.left + (point.clientX - origin.x) / scale;
        const top = rect.top + (point.clientY - origin.y) / scale;
        const dragged = {
          ...rect,
          left: free ? left : snapToGrid(left),
          top: free ? top : snapToGrid(top),
        };

        const placements = usePatchLayoutStore.getState().placements;
        const candidate =
          free || !rects ? null : pinCandidate(targetKey, dragged, rects, placements);

        landing = candidate?.pin ?? null;
        setPinTarget(landing);

        const position = clampToCanvas({ ...dragged, ...(candidate?.at ?? {}) });
        place(targetKey, {
          x: position.x,
          y: position.y,
          pinnedTo: null,
          pinEdge: null,
          ...kept,
        });
      };

      const handleMove = (moveEvent: PointerEvent) => {
        moved = true;
        apply(moveEvent);
      };

      const handleUp = () => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);
        setPinTarget(null);

        if (!moved) {
          return;
        }

        if (landing) {
          void pin(targetKey, landing.anchor, landing.edge);
        } else {
          void save(targetKey);
        }

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
    [targetKey, rect, rects, place, save, pin, setPinTarget]
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
