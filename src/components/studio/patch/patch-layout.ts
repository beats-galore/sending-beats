// Resolving where a node actually goes.
//
// `patch-geometry` computes the arrangement the canvas would make on its own —
// sources down the left, mixes down the middle, destinations down the right.
// This layer sits on top of it and lets a stored placement win, so a node the
// user dragged stays where they put it while everything they have not touched
// goes on being placed.
//
// A placement overrides one part at a time. A node that was only moved keeps
// taking its height from what it is showing, and a node that was only resized
// stays in the column it was stacked into — which is what makes the two
// interactions independent rather than one turning off the other.
import type { PatchPlacement } from '../../../services/patch-layout-service';
import { layout } from '../../../theme/layout';

const { canvas } = layout;

/** How much room something takes. */
export type Size = {
  width: number;
  height: number;
};

/** A node's box in canvas coordinates. */
export type NodeRect = Size & {
  left: number;
  top: number;
};

/** A port, in canvas coordinates. */
export type Port = {
  x: number;
  y: number;
};

/**
 * Where a node goes: whatever the user stored, falling back to the computed
 * arrangement one axis at a time.
 */
export const resolveRect = (
  placement: PatchPlacement | undefined,
  computed: NodeRect
): NodeRect => ({
  left: placement?.x ?? computed.left,
  top: placement?.y ?? computed.top,
  width: placement?.width ?? computed.width,
  height: placement?.height ?? computed.height,
});

/** A port on the left edge, `offset` down from the top. */
export const leftPort = (rect: NodeRect, offset: number): Port => ({
  x: rect.left,
  y: rect.top + offset,
});

/** A port on the right edge, `offset` down from the top. */
export const rightPort = (rect: NodeRect, offset: number): Port => ({
  x: rect.left + rect.width,
  y: rect.top + offset,
});

/**
 * Overall canvas height, from what is actually on it.
 *
 * Nodes can be dragged anywhere, so the canvas can no longer work this out by
 * summing the columns — it has to measure. `bottoms` are the lowest edge of
 * everything drawn, including the targets below the columns, which are not
 * nodes but still need room.
 */
export const canvasHeightOf = (bottoms: number[]): number =>
  Math.max(canvas.minHeight, ...bottoms.map((bottom) => bottom + canvas.bottomPadding));

/**
 * Keeps a node inside the canvas.
 *
 * The canvas is authored at a fixed width and scaled to fit, so a node dragged
 * past the right edge would be scaled off screen with no way to reach it. The
 * bottom has no such limit: the canvas grows downwards and the view scrolls.
 */
export const clampToCanvas = (rect: NodeRect): Port => ({
  x: Math.max(0, Math.min(canvas.width - rect.width, rect.left)),
  y: Math.max(0, rect.top),
});

/**
 * Snaps to the dot grid the canvas is already drawn with, so nodes arranged by
 * hand line up without the user aiming for it.
 */
export const snapToGrid = (value: number): number =>
  Math.round(value / canvas.dotGridSize) * canvas.dotGridSize;

/**
 * Keeps a resize within what a node can actually be.
 *
 * The floor is whatever the node needs when it is showing the least it can, and
 * the ceiling is the canvas — a node wider than the canvas would be scaled off
 * screen along with everything else.
 */
export const clampSize = (size: Size, left: number, minimum: Size): Size => ({
  width: Math.max(minimum.width, Math.min(canvas.width - left, size.width)),
  height: Math.max(minimum.height, size.height),
});
