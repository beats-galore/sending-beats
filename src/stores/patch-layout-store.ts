import { create } from 'zustand';

import type { PatchTargetKey } from '../services/patch-color-service';
import type { PatchPlacement } from '../services/patch-layout-service';
import { EMPTY_PLACEMENT, patchLayoutService } from '../services/patch-layout-service';

type PatchLayoutStore = {
  /** Hand-arranged nodes only. A key absent here is still placed by the canvas. */
  placements: Partial<Record<PatchTargetKey, PatchPlacement>>;
  loaded: boolean;

  /**
   * The node last pressed, which is drawn above the rest.
   *
   * Nodes are drawn in the order the canvas lists them, so stacking one over
   * another leaves the one behind unreachable — clicking it would select it and
   * the one in front would stay in front. Pressing anywhere on a node brings it
   * forward, which is what makes a deliberate stack something you can leaf
   * through, and it lifts a node clear before a drag rather than after.
   *
   * Not stored: the arrangement is remembered, the order it is leafed through
   * is not.
   */
  front: PatchTargetKey | null;
  bringToFront: (targetKey: PatchTargetKey) => void;

  load: () => Promise<void>;
  /** Move or resize a node on screen, without writing anything down. */
  place: (targetKey: PatchTargetKey, patch: Partial<PatchPlacement>) => void;
  /** Write down where a node ended up, once the pointer has been let go. */
  save: (targetKey: PatchTargetKey) => Promise<void>;
  /** Put one node back in its column. */
  reset: (targetKey: PatchTargetKey) => Promise<void>;
  /** Put every node back in its column. */
  tidy: () => Promise<void>;
};

/** Whether anything is actually overridden, or the node is being placed anyway. */
const overridesNothing = (placement: PatchPlacement): boolean =>
  placement.x === null &&
  placement.y === null &&
  placement.width === null &&
  placement.height === null;

export const usePatchLayoutStore = create<PatchLayoutStore>((set, get) => ({
  placements: {},
  loaded: false,

  front: null,
  bringToFront: (front) => set({ front }),

  load: async () => {
    try {
      set({ placements: await patchLayoutService.list(), loaded: true });
    } catch (error) {
      console.error('Failed to load the patch arrangement:', error);
      set({ loaded: true });
    }
  },

  // Dragging a node fires on every pointer move, so this stays in memory and
  // the write is left to `save` on release. Persisting each frame would put a
  // few hundred round trips through a single drag.
  place: (targetKey, patch) =>
    set((state) => ({
      placements: {
        ...state.placements,
        [targetKey]: { ...EMPTY_PLACEMENT, ...state.placements[targetKey], ...patch },
      },
    })),

  save: async (targetKey) => {
    const placement = get().placements[targetKey];
    if (!placement) {
      return;
    }

    try {
      await patchLayoutService.set(targetKey, placement);

      // The backend drops a row that overrides nothing, so a node dragged back
      // to being computed has to leave the map too — otherwise it reads as
      // hand-placed until the next load.
      if (overridesNothing(placement)) {
        set((state) => {
          const placements = { ...state.placements };
          delete placements[targetKey];
          return { placements };
        });
      }
    } catch (error) {
      console.error('Failed to store the patch arrangement:', error);
      await get().load();
    }
  },

  reset: async (targetKey) => {
    const previous = get().placements;
    const placements = { ...previous };
    delete placements[targetKey];
    set({ placements });

    try {
      await patchLayoutService.clear(targetKey);
    } catch (error) {
      console.error('Failed to clear a patch placement:', error);
      set({ placements: previous });
    }
  },

  tidy: async () => {
    const previous = get().placements;
    set({ placements: {} });

    try {
      await patchLayoutService.clearAll();
    } catch (error) {
      console.error('Failed to tidy the patch arrangement:', error);
      set({ placements: previous });
    }
  },
}));
