// Patchbay arrangement service layer - abstraction over Tauri patch layout commands
import { invoke } from '@tauri-apps/api/core';

import type { PatchTargetKey } from './patch-color-service';

/**
 * Where a node sits and how big it is, in canvas coordinates.
 *
 * Every field is nullable because a placement overrides the computed layout
 * rather than replacing it: a node that was dragged but never resized carries a
 * position and no size, and goes on taking its height from what it is showing.
 */
export type PatchPlacement = {
  x: number | null;
  y: number | null;
  width: number | null;
  height: number | null;
};

export const EMPTY_PLACEMENT: PatchPlacement = { x: null, y: null, width: null, height: null };

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
