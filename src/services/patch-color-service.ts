// Patch colour service layer - abstraction over Tauri patch colour commands
import { invoke } from '@tauri-apps/api/core';

import type { Swatch } from '../theme/tokens';

/**
 * What something on the patchbay is stored against, in the backend's own key
 * vocabulary.
 *
 * `ch:<channel number>` for an input strip, `bus:<bus id>` for a mix,
 * `out:<device identifier>` for a hardware destination, `stream` and `rec` for
 * the broadcast and the tape.
 *
 * Shared with `patch-layout-service`, which keys where a node was dragged to
 * the same way: both need to name things that have no common row to store
 * against, and both need those names to survive channels coming and going.
 */
export type PatchTargetKey = string;

export const channelTargetKey = (channelNumber: number): PatchTargetKey =>
  `ch:${channelNumber}`;

export const outputTargetKey = (deviceIdentifier: string): PatchTargetKey =>
  `out:${deviceIdentifier}`;

/** Buses carry no colour of their own, but they are placed like every other node */
export const busTargetKey = (busId: string): PatchTargetKey => `bus:${busId}`;

/** The broadcast and the tape are each the only one of their kind, so they key by name */
export const STREAM_TARGET_KEY: PatchTargetKey = 'stream';
export const TAPE_TARGET_KEY: PatchTargetKey = 'rec';

export const patchColorService = {
  /** Every colour the active session has assigned, keyed by what it colours */
  async list(): Promise<Record<PatchTargetKey, string>> {
    return invoke<Record<PatchTargetKey, string>>('list_patch_colors');
  },

  async set(targetKey: PatchTargetKey, color: Swatch): Promise<void> {
    return invoke('set_patch_color', { targetKey, color });
  },

  /** Forget a colour, so the thing is given a fresh one next time it appears */
  async clear(targetKey: PatchTargetKey): Promise<void> {
    return invoke('clear_patch_color', { targetKey });
  },
} as const;
