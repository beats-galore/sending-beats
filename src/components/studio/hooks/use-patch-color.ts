import { useCallback } from 'react';

import type { PatchTargetKey } from '../../../services/patch-color-service';
import { STREAM_TARGET_KEY, TAPE_TARGET_KEY } from '../../../services/patch-color-service';
import { usePatchColorStore } from '../../../stores/patch-color-store';
import { color, Swatch, swatchColor } from '../../../theme/tokens';

type PatchColor = {
  /** The swatch this thing shows, or null when its colour is reserved */
  key: Swatch | null;
  /** The colour value, for `style` */
  value: string;
  /** Whether the user picked it, or it is only the fallback */
  assigned: boolean;
  /** Fixed by what the thing is, so it offers no picker */
  reserved: boolean;
  select: (key: Swatch) => void;
  reset: () => void;
};

/**
 * Destinations whose colour says what they are rather than which one they are.
 *
 * There is only ever one tape and one broadcast, so a number would count to one
 * and a picker would offer a choice that means nothing. The tape takes the same
 * red it turns while recording, so a tile pointing at it reads as the tape
 * without having to be learned.
 */
const RESERVED: Partial<Record<PatchTargetKey, string>> = {
  [TAPE_TARGET_KEY]: color.hot,
  [STREAM_TARGET_KEY]: color.acc,
};

/** The colour of a reserved destination, or null when the thing picks its own. */
const reservedPatchColor = (targetKey: PatchTargetKey): string | null =>
  RESERVED[targetKey] ?? null;

/**
 * The colour of one thing on the patchbay.
 *
 * Unassigned things still need a colour to draw, and the obvious derivation —
 * `SWATCHES[position]`, as the design prototype does — collides with whatever
 * the user has already chosen. So the search starts at the position and walks
 * to the first swatch nobody holds, which keeps two untouched strips apart
 * while never duplicating a deliberate choice. With every swatch taken it
 * falls back to the position, since some duplication is then unavoidable.
 *
 * Destinations start their search part-way along the palette. Both columns
 * count from zero, so seeding both at the position would paint the first source
 * and the first destination the same colour while neither is assigned — and a
 * tile is meant to say which signal it refers to.
 *
 * @param position where this thing sits in its column, used only for fallback
 */
export const usePatchColor = (targetKey: PatchTargetKey, position: number): PatchColor => {
  const colors = usePatchColorStore((state) => state.colors);
  const setColor = usePatchColorStore((state) => state.setColor);
  const clearColor = usePatchColorStore((state) => state.clearColor);

  const select = useCallback((key: Swatch) => void setColor(targetKey, key), [setColor, targetKey]);
  const reset = useCallback(() => void clearColor(targetKey), [clearColor, targetKey]);

  return { ...patchColorOf(colors, targetKey, position), select, reset };
};

/**
 * The same colour, without the picker attached to it.
 *
 * Split out because a colour is no longer only drawn by the thing it belongs
 * to: the cables and the mix node paint themselves in the colour of whatever
 * they carry, and neither is a component that could hold a hook for every
 * source on the canvas.
 */
export const patchColorOf = (
  colors: Partial<Record<PatchTargetKey, Swatch>>,
  targetKey: PatchTargetKey,
  position: number
): Pick<PatchColor, 'key' | 'value' | 'assigned' | 'reserved'> => {
  const reserved = reservedPatchColor(targetKey);
  if (reserved !== null) {
    return { key: null, value: reserved, assigned: true, reserved: true };
  }

  const assignedKey = colors[targetKey];
  if (assignedKey) {
    return { key: assignedKey, value: swatchColor(assignedKey), assigned: true, reserved: false };
  }

  const taken = new Set(Object.values(colors).filter((key) => key !== undefined));
  const key = derive(position + seedOffset(targetKey), taken);

  return { key, value: swatchColor(key), assigned: false, reserved: false };
};

/** Sources count from the top of the palette, destinations from part-way in. */
const seedOffset = (targetKey: PatchTargetKey): number =>
  targetKey.startsWith('ch:') ? 0 : Math.floor(Swatch.length / 2);

const derive = (seed: number, taken: Set<Swatch>): Swatch => {
  for (let offset = 0; offset < Swatch.length; offset += 1) {
    const candidate = Swatch[(seed + offset) % Swatch.length];
    if (!taken.has(candidate)) {
      return candidate;
    }
  }

  return Swatch[seed % Swatch.length];
};
