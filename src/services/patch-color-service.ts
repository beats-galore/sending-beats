// Patch colour service layer - abstraction over Tauri patch colour commands
import { invoke } from '@tauri-apps/api/core';

import type { Swatch } from '../theme/tokens';

/**
 * What a colour is stored against, in the backend's own key vocabulary.
 *
 * `ch:<channel number>` for an input strip, `out:<device identifier>` for a
 * hardware destination, `stream` and `rec` for the broadcast and the tape.
 */
export type PatchTargetKey = string;

export const channelTargetKey = (channelNumber: number): PatchTargetKey =>
  `ch:${channelNumber}`;

export const outputTargetKey = (deviceIdentifier: string): PatchTargetKey =>
  `out:${deviceIdentifier}`;

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
