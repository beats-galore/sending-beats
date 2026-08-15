import { useCallback } from 'react';

import type { PointerEvent as ReactPointerEvent } from 'react';
import type { PatchTargetKey } from '../../../services/patch-color-service';
import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import type { NodeExpansion } from '../patch/patch-geometry';
import { clampSize, snapToGrid } from '../patch/patch-layout';
import type { NodeRect, Size } from '../patch/patch-layout';
import { pinnedCluster } from '../patch/patch-pins';
import type { PatchRects } from '../patch/patch-rects';
import { usePatchRectsContext } from '../patch/patch-rects-context';

/**
 * The grip's own size in canvas coordinates.
 *
 * Also how the scale is recovered: the canvas is authored at a fixed width and
 * scaled to fit, so the grip's width on screen against the width it was drawn
 * at is exactly the factor pointer movement has to be divided by.
 */
export const RESIZE_GRIP_SIZE = 16;

/**
 * Everything sized along with this node.
 *
 * A pinned group is drawn as one block, so it is sized as one: reaching for the
 * toggle or the corner of any card in a group acts on the block rather than on
 * the one card, which is the only reading that makes sense once they are joined
 * with no seam between them.
 *
 * On its own, a node is its own group of one.
 */
const sizedWith = (targetKey: PatchTargetKey, rects: PatchRects | null): PatchTargetKey[] => {
  if (!rects) {
    return [targetKey];
  }

  return pinnedCluster(targetKey, rects.keys, usePatchLayoutStore.getState().placements);
};

/**
 * Sets a node's size outright, and its group with it.
 *
 * For the controls that change how much a node has to show rather than how much
 * of it to show: switching a channel's effects on needs room for the chain, and
 * switching them off gives that room back rather than leaving the card standing
 * over an empty half of itself.
 */
export const useNodeSize = (targetKey: PatchTargetKey) => {
  const place = usePatchLayoutStore((state) => state.place);
  const save = usePatchLayoutStore((state) => state.save);
  const rects = usePatchRectsContext();

  return useCallback(
    (size: Size) => {
      for (const key of sizedWith(targetKey, rects)) {
        place(key, size);
        void save(key);
      }
    },
    [targetKey, rects, place, save]
  );
};

/**
 * Steps a node to one of its three rungs, and its group with it.
 *
 * A group takes the size its hungriest member needs for that rung, not the size
 * the card that was pressed needs. Members are not the same shape — an
 * application card carries a track readout a hardware one has no use for, and a
 * mix opened is not the size a source opened is — and a group is drawn as one
 * block, so anything less than the largest leaves somebody showing less than
 * the rung asked for.
 */
export const useNodeRung = (targetKey: PatchTargetKey) => {
  const place = usePatchLayoutStore((state) => state.place);
  const save = usePatchLayoutStore((state) => state.save);
  const rects = usePatchRectsContext();

  return useCallback(
    (rung: NodeExpansion) => {
      const group = sizedWith(targetKey, rects);
      const wanted = group.flatMap((key) => {
        const size = rects?.ladders[key]?.[rung];
        return size ? [size] : [];
      });

      if (wanted.length === 0) {
        return;
      }

      const size = {
        width: Math.max(...wanted.map((entry) => entry.width)),
        height: Math.max(...wanted.map((entry) => entry.height)),
      };

      for (const key of group) {
        place(key, size);
        void save(key);
      }
    },
    [targetKey, rects, place, save]
  );
};

/**
 * Reaching for a node that has been shrunk out of the way brings it back.
 *
 * A node shrunk that far is showing one reading and nothing else, so clicking
 * it is a request to read it — the click that focuses it gives it the room to
 * answer. Every larger size is left alone: a click on a node you can already
 * read is not a request to resize it.
 *
 * A drag ends in a click on the node it moved, but that one is swallowed before
 * it lands, so shoving a shrunk node across the canvas does not also open it.
 */
export const useUnshrink = (targetKey: PatchTargetKey, shrunk: boolean) => {
  const setRung = useNodeRung(targetKey);

  return () => {
    if (shrunk) {
      setRung('collapsed');
    }
  };
};

/**
 * Makes the corner grip resize its node, and its group with it.
 *
 * Both axes, because how open a node is follows from how big it is — dragging a
 * source taller is the same gesture as opening it, and the toggle is only a
 * shortcut to the size it would have been dragged to. Every card in a pinned
 * group takes the same size, so dragging any corner resizes the block.
 *
 * Sizes snap to the same dot grid positions do, and ⌥ bypasses it the same way.
 * Nothing is written until the pointer is released.
 */
export const useNodeResize = (targetKey: PatchTargetKey, rect: NodeRect, minimum: Size) => {
  const place = usePatchLayoutStore((state) => state.place);
  const save = usePatchLayoutStore((state) => state.save);
  const rects = usePatchRectsContext();

  return useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      if (event.button !== 0) {
        return;
      }

      // The node's title bar drags it, and a resize is not a drag.
      event.stopPropagation();
      // Without this the resize sweeps a text selection across the node it is
      // resizing, and every label inside comes up highlighted.
      event.preventDefault();

      const grip = event.currentTarget.getBoundingClientRect();
      const scale = grip.width > 0 ? grip.width / RESIZE_GRIP_SIZE : 1;
      const origin = { x: event.clientX, y: event.clientY };
      const group = sizedWith(targetKey, rects);
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

        for (const key of group) {
          place(key, size);
        }
      };

      const handleMove = (moveEvent: PointerEvent) => {
        moved = true;
        apply(moveEvent);
      };

      const handleUp = () => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);

        if (moved) {
          for (const key of group) {
            void save(key);
          }
        }
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
    },
    [targetKey, rect, minimum, rects, place, save]
  );
};
