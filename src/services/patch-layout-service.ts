// Patchbay arrangement service layer - abstraction over Tauri patch layout commands
import { invoke } from '@tauri-apps/api/core';

import type { PatchTargetKey } from './patch-color-service';

/**
 * Which edge of its anchor a pinned node sits against.
 *
 * There is no `top`: pinning A above B and pinning B below A are the same
 * arrangement, and two ways to say one thing means two code paths that have to
 * agree about it.
 */
export const PinEdge = ['bottom', 'left', 'right'] as const;
export type PinEdge = (typeof PinEdge)[number];

export const isPinEdge = (value: unknown): value is PinEdge =>
  typeof value === 'string' && PinEdge.includes(value as PinEdge);

/**
 * Where a node sits and how big it is, in canvas coordinates.
 *
 * Every field is nullable because a placement overrides the computed layout
 * rather than replacing it: a node that was dragged but never resized carries a
 * position and no size, and goes on taking its height from what it is showing.
 *
 * A pinned node takes its position from the node `pinnedTo` names rather than
 * from `x` and `y`, which is what carries a whole group when its anchor moves.
 */
export type PatchPlacement = {
  x: number | null;
  y: number | null;
  width: number | null;
  height: number | null;
  /** Target key of the node this one sits against, if any. */
  pinnedTo: string | null;
  pinEdge: PinEdge | null;
};

export const EMPTY_PLACEMENT: PatchPlacement = {
  x: null,
  y: null,
  width: null,
  height: null,
  pinnedTo: null,
  pinEdge: null,
};

export const patchLayoutService = {
  /** Everywhere the active session has put something, keyed by what it places */
  async list(): Promise<Record<PatchTargetKey, PatchPlacement>> {
    return invoke<Record<PatchTargetKey, PatchPlacement>>('list_patch_layouts');
  },

  async set(targetKey: PatchTargetKey, placement: PatchPlacement): Promise<void> {
    return invoke('set_patch_layout', { targetKey, placement });
  },

  /** Forget where one node was put, so the canvas places it again */
  async clear(targetKey: PatchTargetKey): Promise<void> {
    return invoke('clear_patch_layout', { targetKey });
  },

  /** Forget the whole arrangement — what "tidy" does */
  async clearAll(): Promise<void> {
    return invoke('clear_patch_layouts');
  },
} as const;
