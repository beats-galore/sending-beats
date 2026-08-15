// Placing the nodes that have been pinned to another node's edge.
//
// A pin makes one node take its position from another instead of from its own
// stored one, which is what carries a whole group when its anchor is dragged —
// no group-move code, just a position that derives.
//
// Everything here is pure and defensive. Pins are stored by target key against
// things that come and go, so an anchor may simply not be on the canvas any
// more; a node pinned to nothing is placed as though it were never pinned.
import type { PatchTargetKey } from '../../../services/patch-color-service';
import type { PatchPlacement, PinEdge } from '../../../services/patch-layout-service';
import type { NodeRect, Size } from './patch-layout';

/** How far a chain of pins is followed before it is treated as a loop. */
const MAX_CHAIN = 32;

export type Pin = {
  anchor: PatchTargetKey;
  edge: PinEdge;
};

/**
 * The pin on a placement, once both halves of it are actually there.
 *
 * `self` is the node the placement belongs to, so a row that somehow claims to
 * be pinned to itself reads as no pin — there would be no anchor to derive a
 * position from, and every guard downstream would have to know that.
 */
export const pinOf = (
  placement: PatchPlacement | undefined,
  self?: PatchTargetKey
): Pin | null =>
  placement?.pinnedTo && placement.pinEdge && placement.pinnedTo !== self
    ? { anchor: placement.pinnedTo, edge: placement.pinEdge }
    : null;

/**
 * Where a node sits against an edge of its anchor.
 *
 * Flush, with no gap: a gap would read as two nodes that happen to be near each
 * other, and being distinguishable from that is the whole point of a pin.
 *
 * `offset` steps a node past any already sharing the edge, always downwards, so
 * pinning three nodes to one edge builds the same column that chaining them
 * would rather than piling them in one spot.
 */
export const pinnedAt = (
  anchor: NodeRect,
  edge: PinEdge,
  size: Size,
  offset: number
): { left: number; top: number } => {
  switch (edge) {
    case 'bottom':
      return { left: anchor.left, top: anchor.top + anchor.height + offset };
    case 'left':
      return { left: anchor.left - size.width, top: anchor.top + offset };
    case 'right':
      return { left: anchor.left + anchor.width, top: anchor.top + offset };
  }
};

type PinResolution = {
  /** Every node on the canvas, in the order same-edge stacking should follow. */
  keys: PatchTargetKey[];
  rects: Record<PatchTargetKey, NodeRect>;
  placements: Partial<Record<PatchTargetKey, PatchPlacement>>;
};

/**
 * Every node's box with pins applied, in place of the positions they had.
 *
 * Anchors are resolved before their followers, so a chain lands in one pass
 * however the keys happen to be ordered. A node that turns out to be its own
 * ancestor keeps the position it already had rather than being sent somewhere
 * arbitrary — a loop is a mistake to survive, not to draw.
 */
export const applyPins = ({
  keys,
  rects,
  placements,
}: PinResolution): Record<PatchTargetKey, NodeRect> => {
  const resolved: Record<PatchTargetKey, NodeRect> = { ...rects };
  const placed = new Set<PatchTargetKey>();
  // How much of each edge is already spoken for, so the next node to claim it
  // stacks below rather than over.
  const claimed: Record<string, number> = {};

  const place = (key: PatchTargetKey, ancestors: Set<PatchTargetKey>): NodeRect => {
    if (placed.has(key)) {
      return resolved[key];
    }
    // Reached through its own followers, or down an unreasonably long chain.
    // Left where it is, and deliberately not marked done, so the call that owns
    // it can still place it.
    if (ancestors.has(key) || ancestors.size > MAX_CHAIN) {
      return resolved[key];
    }

    const pin = pinOf(placements[key], key);
    if (!pin || !(pin.anchor in resolved)) {
      placed.add(key);
      return resolved[key];
    }

    ancestors.add(key);
    const anchor = place(pin.anchor, ancestors);

    const rect = resolved[key];
    const slot = `${pin.anchor}:${pin.edge}`;
    const offset = claimed[slot] ?? 0;
    claimed[slot] = offset + rect.height;

    resolved[key] = { ...rect, ...pinnedAt(anchor, pin.edge, rect, offset) };
    placed.add(key);

    return resolved[key];
  };

  for (const key of keys) {
    place(key, new Set());
  }

  return resolved;
};

/** Every node that would move with this one, including itself. */
export const pinnedGroup = (
  anchor: PatchTargetKey,
  keys: PatchTargetKey[],
  placements: Partial<Record<PatchTargetKey, PatchPlacement>>
): PatchTargetKey[] => {
  const group = new Set<PatchTargetKey>([anchor]);

  // Followers of followers come along too, so a chain moves whole. Repeated
  // until nothing new is found rather than recursing, which keeps a loop from
  // running away.
  for (let pass = 0; pass < MAX_CHAIN; pass += 1) {
    const before = group.size;
    for (const key of keys) {
      const pin = pinOf(placements[key], key);
      if (pin && group.has(pin.anchor)) {
        group.add(key);
      }
    }
    if (group.size === before) {
      break;
    }
  }

  return [...group];
};
