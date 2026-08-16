import { create } from 'zustand';
import { persist } from 'zustand/middleware';

import { filePlayerService } from '../services/file-player-service';
import { queueService } from '../services/queue-service';
import type { FilePlayer, Queue, QueuePlay, QueueTrack } from '../types/file-player.types';
import type { FilePath, Uuid } from '../types/util.types';

type QueueId = Uuid<FilePlayer>;

/** What the first queue is called, before there is anything to number it against. */
const FIRST_QUEUE_NAME = 'Queue';

type QueueStore = {
  queues: Queue[];
  /** Which queue the queues screen is looking at */
  selectedId: QueueId | null;
  /** Queues on the current patch's canvas, by id */
  targetIds: QueueId[];
  /** What is in the selected queue, and what it has played */
  tracks: QueueTrack[];
  plays: QueuePlay[];
  loaded: boolean;

  load: () => Promise<void>;
  select: (id: QueueId) => Promise<void>;
  refreshSelected: () => Promise<void>;

  add: () => Promise<QueueId | null>;
  rename: (id: QueueId, name: string) => Promise<void>;
  remove: (id: QueueId) => Promise<void>;

  addTracks: (id: QueueId, filePaths: FilePath[]) => Promise<void>;
  browseForTracks: (id: QueueId) => Promise<void>;
  clearPlays: (id: QueueId) => Promise<void>;

  addTarget: (id: QueueId) => Promise<void>;
  removeTarget: (id: QueueId) => Promise<void>;
};

export const useQueueStore = create<QueueStore>()(
  persist(
    (set, get) => ({
      queues: [],
      selectedId: null,
      targetIds: [],
      tracks: [],
      plays: [],
      loaded: false,

      load: async () => {
        try {
          const [queues, targetIds] = await Promise.all([
            queueService.list(),
            queueService.listTargets(),
          ]);

          // A selection is remembered across launches, but the queue behind it
          // may have been deleted since — falling back to the first keeps the
          // screen pointed at something real.
          const { selectedId } = get();
          const stillThere = queues.some((entry) => entry.id === selectedId);
          const nextId = stillThere ? selectedId : (queues.at(0)?.id ?? null);

          set({ queues, targetIds, selectedId: nextId, loaded: true });

          if (nextId) {
            await get().refreshSelected();
          }
        } catch (error) {
          console.error('Failed to load queues:', error);
          set({ loaded: true });
        }
      },

      select: async (id) => {
        set({ selectedId: id, tracks: [], plays: [] });
        await get().refreshSelected();
      },

      /** Read what the selected queue holds and what it has played. */
      refreshSelected: async () => {
        const { selectedId } = get();
        if (!selectedId) {
          set({ tracks: [], plays: [] });
          return;
        }

        try {
          const [tracks, plays] = await Promise.all([
            queueService.tracks(selectedId),
            queueService.plays(selectedId),
          ]);
          set({ tracks, plays });
        } catch (error) {
          console.error('Failed to read the queue:', error);
        }
      },

      add: async () => {
        const { queues } = get();
        const name =
          queues.length === 0 ? FIRST_QUEUE_NAME : `${FIRST_QUEUE_NAME} ${queues.length + 1}`;

        try {
          // Created through the player service: a queue is a running player as
          // well as a row, and one made without the other cannot be patched in.
          const id = await filePlayerService.create(name);
          const created = await queueService.list();

          set({ queues: created, selectedId: id, tracks: [], plays: [] });
          return id;
        } catch (error) {
          console.error('Failed to create the queue:', error);
          return null;
        }
      },

      rename: async (id, name) => {
        const previous = get().queues;

        // Applied locally first: this is a text field being typed into, and a
        // round trip per keystroke would fight the cursor.
        set({
          queues: previous.map((entry) => (entry.id === id ? { ...entry, name } : entry)),
        });

        try {
          await queueService.rename(id, name);
        } catch (error) {
          console.error('Failed to rename the queue:', error);
          set({ queues: previous });
        }
      },

      remove: async (id) => {
        try {
          await filePlayerService.remove(id);
          set((state) => {
            const queues = state.queues.filter((entry) => entry.id !== id);
            return {
              queues,
              // The row is gone, so no patch can be pointed at it either.
              targetIds: state.targetIds.filter((entry) => entry !== id),
              selectedId: state.selectedId === id ? (queues.at(0)?.id ?? null) : state.selectedId,
            };
          });
          await get().refreshSelected();
        } catch (error) {
          console.error('Failed to delete the queue:', error);
        }
      },

      addTracks: async (id, filePaths) => {
        try {
          // One at a time, in order: the backend appends, and firing them
          // together would queue an album in whatever order they finished.
          for (const filePath of filePaths) {
            await filePlayerService.addTrack(id, filePath);
          }
        } catch (error) {
          console.error('Failed to add tracks:', error);
        }

        await get().refreshSelected();
      },

      browseForTracks: async (id) => {
        try {
          const picked = await filePlayerService.browse();
          if (picked.length > 0) {
            await get().addTracks(id, picked);
          }
        } catch (error) {
          console.error('Failed to browse for tracks:', error);
        }
      },

      clearPlays: async (id) => {
        try {
          await queueService.clearPlays(id);
        } catch (error) {
          console.error('Failed to clear the play log:', error);
        }

        await get().refreshSelected();
      },

      addTarget: async (id) => {
        const previous = get().targetIds;
        if (previous.includes(id)) {
          return;
        }

        set({ targetIds: [...previous, id] });

        try {
          await queueService.addTarget(id);
        } catch (error) {
          console.error('Failed to put the queue on this patch:', error);
          set({ targetIds: previous });
        }
      },

      removeTarget: async (id) => {
        const previous = get().targetIds;
        set({ targetIds: previous.filter((entry) => entry !== id) });

        try {
          await queueService.removeTarget(id);
        } catch (error) {
          console.error('Failed to take the queue off this patch:', error);
          set({ targetIds: previous });
        }
      },
    }),
    {
      name: 'sweet-beats-studio-queues',
      // Only the choice is remembered. The queues themselves are in the
      // database, and a stale copy here would argue with it.
      partialize: (state) => ({ selectedId: state.selectedId }),
    }
  )
);

/** The queue the screen is looking at, or null when there are none. */
export const selectedQueue = (state: QueueStore): Queue | null =>
  state.queues.find((entry) => entry.id === state.selectedId) ?? null;
