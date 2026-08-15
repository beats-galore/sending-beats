import { create } from 'zustand';

import type { PatchTargetKey } from '../services/patch-color-service';
import { patchColorService } from '../services/patch-color-service';
import type { Swatch } from '../theme/tokens';
import { isSwatch } from '../theme/tokens';

type PatchColorStore = {
  /** Assigned colours only. A key absent here has never been given one. */
  colors: Partial<Record<PatchTargetKey, Swatch>>;
  loaded: boolean;

  load: () => Promise<void>;
  setColor: (targetKey: PatchTargetKey, color: Swatch) => Promise<void>;
  clearColor: (targetKey: PatchTargetKey) => Promise<void>;
};

export const usePatchColorStore = create<PatchColorStore>((set, get) => ({
  colors: {},
  loaded: false,

  load: async () => {
    try {
      const stored = await patchColorService.list();

      // A row written by an older palette, or by hand, names a swatch this
      // build no longer has. Dropping it is what makes the strip fall back to a
      // derived colour rather than render nothing.
      const colors: Partial<Record<PatchTargetKey, Swatch>> = {};
      for (const [targetKey, value] of Object.entries(stored)) {
        if (isSwatch(value)) {
          colors[targetKey] = value;
        }
      }

      set({ colors, loaded: true });
    } catch (error) {
      console.error('Failed to load patch colours:', error);
      set({ loaded: true });
    }
  },

  setColor: async (targetKey, color) => {
    const previous = get().colors;
    set({ colors: { ...previous, [targetKey]: color } });

    try {
      await patchColorService.set(targetKey, color);
    } catch (error) {
      console.error('Failed to store patch colour:', error);
      set({ colors: previous });
    }
  },

  clearColor: async (targetKey) => {
    const previous = get().colors;
    const colors = { ...previous };
    delete colors[targetKey];
    set({ colors });

    try {
      await patchColorService.clear(targetKey);
    } catch (error) {
      console.error('Failed to clear patch colour:', error);
      set({ colors: previous });
    }
  },
}));
