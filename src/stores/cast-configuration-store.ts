import { create } from 'zustand';
import { persist } from 'zustand/middleware';

import { castConfigurationService } from '../services/cast-configuration-service';
import type { CastConfiguration, CastConfigurationInput } from '../types/cast.types';
import { DEFAULT_CAST_INPUT } from '../types/cast.types';

type CastConfigurationStore = {
  configurations: CastConfiguration[];
  /** Which station the transmitter is pointed at */
  selectedId: string | null;
  /** Stations on the current patch's canvas, by id */
  targetIds: string[];
  loaded: boolean;

  load: () => Promise<void>;
  addTarget: (id: string) => Promise<void>;
  removeTarget: (id: string) => Promise<void>;
  select: (id: string) => void;
  add: () => Promise<CastConfiguration | null>;
  update: (id: string, input: CastConfigurationInput) => Promise<void>;
  remove: (id: string) => Promise<void>;
  setPassword: (id: string, password: string) => Promise<void>;
};

export const useCastConfigurationStore = create<CastConfigurationStore>()(
  persist(
    (set, get) => ({
      configurations: [],
      selectedId: null,
      targetIds: [],
      loaded: false,

      load: async () => {
        try {
          const [configurations, targetIds] = await Promise.all([
            castConfigurationService.list(),
            castConfigurationService.listTargets(),
          ]);

          // A selection is remembered across launches, but the station behind it
          // may have been deleted since — falling back to the first keeps the
          // transmitter pointed at something real.
          const { selectedId } = get();
          const stillThere = configurations.some((entry) => entry.id === selectedId);

          set({
            configurations,
            targetIds,
            selectedId: stillThere ? selectedId : (configurations.at(0)?.id ?? null),
            loaded: true,
          });
        } catch (error) {
          console.error('Failed to load cast configurations:', error);
          set({ loaded: true });
        }
      },

      select: (id) => set({ selectedId: id }),

      addTarget: async (id) => {
        const previous = get().targetIds;
        if (previous.includes(id)) {
          return;
        }

        set({ targetIds: [...previous, id] });

        try {
          await castConfigurationService.addTarget(id);
        } catch (error) {
          console.error('Failed to add the cast destination:', error);
          set({ targetIds: previous });
        }
      },

      removeTarget: async (id) => {
        const previous = get().targetIds;
        set({ targetIds: previous.filter((entry) => entry !== id) });

        try {
          await castConfigurationService.removeTarget(id);
        } catch (error) {
          console.error('Failed to remove the cast destination:', error);
          set({ targetIds: previous });
        }
      },

      add: async () => {
        try {
          const created = await castConfigurationService.create(DEFAULT_CAST_INPUT);
          set((state) => ({
            configurations: [...state.configurations, created],
            // Selected straight away, since adding one is how you say you want
            // to work on it.
            selectedId: created.id,
          }));
          return created;
        } catch (error) {
          console.error('Failed to create cast configuration:', error);
          return null;
        }
      },

      update: async (id, input) => {
        const previous = get().configurations;

        // Applied locally first: these are text fields being typed into, and a
        // round trip per keystroke would fight the cursor.
        set({
          configurations: previous.map((entry) =>
            entry.id === id ? { ...entry, ...input } : entry
          ),
        });

        try {
          const updated = await castConfigurationService.update(id, input);
          set((state) => ({
            configurations: state.configurations.map((entry) =>
              entry.id === id ? updated : entry
            ),
          }));
        } catch (error) {
          console.error('Failed to save cast configuration:', error);
          set({ configurations: previous });
        }
      },

      remove: async (id) => {
        try {
          await castConfigurationService.remove(id);
          set((state) => {
            const configurations = state.configurations.filter((entry) => entry.id !== id);
            return {
              configurations,
              // The row is gone, so the canvas cannot be pointed at it either.
              targetIds: state.targetIds.filter((entry) => entry !== id),
              selectedId:
                state.selectedId === id ? (configurations.at(0)?.id ?? null) : state.selectedId,
            };
          });
        } catch (error) {
          console.error('Failed to delete cast configuration:', error);
        }
      },

      setPassword: async (id, password) => {
        try {
          const hasPassword = await castConfigurationService.setPassword(id, password);
          set((state) => ({
            configurations: state.configurations.map((entry) =>
              entry.id === id ? { ...entry, hasPassword } : entry
            ),
          }));
        } catch (error) {
          console.error('Failed to store the stream password:', error);
        }
      },
    }),
    {
      name: 'sweet-beats-studio-cast',
      // Only the choice is remembered. The stations themselves are in the
      // database, and a stale copy here would argue with it.
      partialize: (state) => ({ selectedId: state.selectedId }),
    }
  )
);

/** The station the transmitter is pointed at, or null when there are none. */
export const selectedCastConfiguration = (
  state: CastConfigurationStore
): CastConfiguration | null =>
  state.configurations.find((entry) => entry.id === state.selectedId) ?? null;
