import { create } from 'zustand';

import { filePlayerService } from '../services/file-player-service';
import type {
  FilePlayer,
  PlaybackAction,
  PlaybackStatus,
  QueuedTrack,
} from '../types/file-player.types';
import type { FilePath, Uuid } from '../types/util.types';

type PlayerId = Uuid<FilePlayer>;
type TrackId = Uuid<QueuedTrack>;

type FilePlayerStore = {
  players: FilePlayer[];
  /**
   * What each player has waiting, and what each is doing, by player id.
   *
   * Partial because a player is in the list before its first read comes back —
   * asking for one that has not been read yet has to be a real answer rather
   * than an entry the type system claims is there.
   */
  queues: Partial<Record<PlayerId, QueuedTrack[]>>;
  statuses: Partial<Record<PlayerId, PlaybackStatus>>;
  loaded: boolean;

  load: () => Promise<void>;
  create: (name: string) => Promise<PlayerId | null>;
  remove: (playerId: PlayerId) => Promise<void>;
  refresh: (playerId: PlayerId) => Promise<void>;
  poll: () => Promise<void>;

  addTracks: (playerId: PlayerId, filePaths: FilePath[]) => Promise<void>;
  removeTrack: (playerId: PlayerId, trackId: TrackId) => Promise<void>;
  moveTrack: (playerId: PlayerId, trackId: TrackId, toIndex: number) => Promise<void>;
  clearQueue: (playerId: PlayerId) => Promise<void>;
  setBreakpoint: (playerId: PlayerId, trackId: TrackId | null) => Promise<void>;
  control: (playerId: PlayerId, action: PlaybackAction) => Promise<void>;
};

export const useFilePlayerStore = create<FilePlayerStore>((set, get) => ({
  players: [],
  queues: {},
  statuses: {},
  loaded: false,

  /**
   * Bring back this session's players, then read what each is holding.
   *
   * Restoring first: the players are stored against the configuration, and the
   * ones running are only whatever this launch has already created.
   */
  load: async () => {
    try {
      await filePlayerService.restore();
      const players = await filePlayerService.list();
      set({ players, loaded: true });

      await Promise.all(players.map((player) => get().refresh(player.id)));
    } catch (error) {
      console.error('Failed to load file players:', error);
      set({ loaded: true });
    }
  },

  create: async (name) => {
    try {
      const playerId = await filePlayerService.create(name);
      set((state) => ({
        players: [...state.players, { id: playerId, name }],
        queues: { ...state.queues, [playerId]: [] },
      }));
      return playerId;
    } catch (error) {
      console.error('Failed to create the player:', error);
      return null;
    }
  },

  remove: async (playerId) => {
    try {
      await filePlayerService.remove(playerId);
      set((state) => {
        const queues = { ...state.queues };
        const statuses = { ...state.statuses };
        delete queues[playerId];
        delete statuses[playerId];

        return {
          players: state.players.filter((player) => player.id !== playerId),
          queues,
          statuses,
        };
      });
    } catch (error) {
      console.error('Failed to remove the player:', error);
    }
  },

  /** Read one player's queue and what it is doing with it. */
  refresh: async (playerId) => {
    try {
      const [queue, status] = await Promise.all([
        filePlayerService.queue(playerId),
        filePlayerService.status(playerId),
      ]);

      set((state) => ({
        queues: { ...state.queues, [playerId]: queue },
        statuses: { ...state.statuses, [playerId]: status },
      }));
    } catch (error) {
      console.error(`Failed to read player ${playerId}:`, error);
    }
  },

  /**
   * The playhead and the state of every player.
   *
   * Only the status, not the queue: the playhead moves on its own and has to be
   * asked for, where the queue only changes when something here changes it.
   */
  poll: async () => {
    const { players } = get();
    if (players.length === 0) {
      return;
    }

    const read = await Promise.all(
      players.map(async (player) => {
        try {
          return [player.id, await filePlayerService.status(player.id)] as const;
        } catch {
          return null;
        }
      })
    );

    set((state) => ({
      statuses: {
        ...state.statuses,
        ...Object.fromEntries(read.filter((entry) => entry !== null)),
      },
    }));
  },

  addTracks: async (playerId, filePaths) => {
    try {
      // One at a time, in order: the backend appends, and firing them together
      // would queue an album in whatever order the reads happened to finish.
      for (const filePath of filePaths) {
        await filePlayerService.addTrack(playerId, filePath);
      }
    } catch (error) {
      console.error('Failed to add tracks to the queue:', error);
    }

    await get().refresh(playerId);
  },

  removeTrack: async (playerId, trackId) => {
    const previous = get().queues[playerId] ?? [];
    set((state) => ({
      queues: {
        ...state.queues,
        [playerId]: previous.filter((track) => track.id !== trackId),
      },
    }));

    try {
      await filePlayerService.removeTrack(playerId, trackId);
      await get().refresh(playerId);
    } catch (error) {
      console.error('Failed to remove the track:', error);
      set((state) => ({ queues: { ...state.queues, [playerId]: previous } }));
    }
  },

  moveTrack: async (playerId, trackId, toIndex) => {
    const previous = get().queues[playerId] ?? [];
    const from = previous.findIndex((track) => track.id === trackId);
    if (from === -1 || toIndex < 0 || toIndex >= previous.length) {
      return;
    }

    // Applied here first, because this is a list being nudged a row at a time
    // and a round trip per press would make it stutter.
    const reordered = [...previous];
    const [moved] = reordered.splice(from, 1);
    reordered.splice(toIndex, 0, moved);
    set((state) => ({ queues: { ...state.queues, [playerId]: reordered } }));

    try {
      await filePlayerService.moveTrack(playerId, trackId, toIndex);
    } catch (error) {
      console.error('Failed to reorder the queue:', error);
      set((state) => ({ queues: { ...state.queues, [playerId]: previous } }));
    }
  },

  clearQueue: async (playerId) => {
    try {
      await filePlayerService.clearQueue(playerId);
    } catch (error) {
      console.error('Failed to clear the queue:', error);
    }

    await get().refresh(playerId);
  },

  setBreakpoint: async (playerId, trackId) => {
    const previous = get().statuses[playerId];
    if (previous) {
      set((state) => ({
        statuses: {
          ...state.statuses,
          [playerId]: { ...previous, breakpointTrackId: trackId },
        },
      }));
    }

    try {
      await filePlayerService.setBreakpoint(playerId, trackId);
    } catch (error) {
      console.error('Failed to set the breakpoint:', error);
      if (previous) {
        set((state) => ({ statuses: { ...state.statuses, [playerId]: previous } }));
      }
    }
  },

  control: async (playerId, action) => {
    try {
      await filePlayerService.control(playerId, action);
    } catch (error) {
      console.error('Failed to control the player:', error);
    }

    await get().refresh(playerId);
  },
}));
