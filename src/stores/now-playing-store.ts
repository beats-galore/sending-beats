import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { create } from 'zustand';

import type {
  NowPlayingErrorEvent,
  NowPlayingEvent,
  NowPlayingPlayerInfo,
  NowPlayingTrack,
} from '../types/now-playing.types';
import { NOW_PLAYING_CHANGED_EVENT, NOW_PLAYING_ERROR_EVENT } from '../types/now-playing.types';

type NowPlayingStore = {
  /** Current track per bundle identifier; absent means nothing is playing. */
  tracks: Record<string, NowPlayingTrack>;
  /** Last failure per bundle identifier, most often denied Automation consent. */
  errors: Record<string, string>;
  /**
   * Applications the backend can read a track from. An application source
   * outside this list is captured like any other but has no metadata to show.
   */
  supportedBundleIds: string[];
  subscribed: boolean;

  subscribe: () => Promise<void>;
};

export const useNowPlayingStore = create<NowPlayingStore>((set, get) => ({
  tracks: {},
  errors: {},
  supportedBundleIds: [],
  subscribed: false,

  /**
   * Attach to the backend watcher. The watcher itself decides what to poll —
   * only applications configured as inputs — so this just listens and starts it.
   */
  subscribe: async () => {
    if (get().subscribed) {
      return;
    }
    set({ subscribed: true });

    await listen<NowPlayingEvent>(NOW_PLAYING_CHANGED_EVENT, ({ payload }) => {
      set((state) => {
        const tracks = { ...state.tracks };
        if (payload.track) {
          tracks[payload.bundleId] = payload.track;
        } else {
          delete tracks[payload.bundleId];
        }
        const errors = { ...state.errors };
        delete errors[payload.bundleId];
        return { tracks, errors };
      });
    });

    await listen<NowPlayingErrorEvent>(NOW_PLAYING_ERROR_EVENT, ({ payload }) => {
      set((state) => ({
        errors: { ...state.errors, [payload.bundleId]: payload.message },
      }));
    });

    try {
      const players = await invoke<NowPlayingPlayerInfo[]>('list_now_playing_players');
      set({ supportedBundleIds: players.map((player) => player.bundleId) });
      await invoke('start_now_playing_watch');
    } catch (error) {
      console.error('Failed to start now-playing watch:', error);
    }
  },
}));
