import { useCallback } from 'react';

import type { PatchTargetKey } from '../../../services/patch-color-service';
import { usePatchColorStore } from '../../../stores/patch-color-store';
import { Swatch, swatchColor } from '../../../theme/tokens';

type PatchColor = {
  /** The swatch this thing shows, assigned or derived */
  key: Swatch;
  /** That swatch as a colour value, for `style` */
  value: string;
  /** Whether the user picked it, or it is only the fallback */
  assigned: boolean;
  select: (key: Swatch) => void;
  reset: () => void;
};

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
 * @param position where this thing sits in its column, used only for fallback
 */
export const usePatchColor = (targetKey: PatchTargetKey, position: number): PatchColor => {
  const colors = usePatchColorStore((state) => state.colors);
  const setColor = usePatchColorStore((state) => state.setColor);
  const clearColor = usePatchColorStore((state) => state.clearColor);

  const select = useCallback(
    (key: Swatch) => void setColor(targetKey, key),
    [setColor, targetKey]
  );
  const reset = useCallback(() => void clearColor(targetKey), [clearColor, targetKey]);

  const assignedKey = colors[targetKey];
  if (assignedKey) {
    return { key: assignedKey, value: swatchColor(assignedKey), assigned: true, select, reset };
  }

  const taken = new Set(Object.values(colors).filter((key) => key !== undefined));
  const key = derive(position, taken);

  return { key, value: swatchColor(key), assigned: false, select, reset };
};

const derive = (position: number, taken: Set<Swatch>): Swatch => {
  for (let offset = 0; offset < Swatch.length; offset += 1) {
    const candidate = Swatch[(position + offset) % Swatch.length];
    if (!taken.has(candidate)) {
      return candidate;
    }
  }

  return Swatch[position % Swatch.length];
};
