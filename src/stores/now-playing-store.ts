import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { create } from 'zustand';

import type {
  NowPlayingErrorEvent,
  NowPlayingEvent,
  NowPlayingTrack,
} from '../types/now-playing.types';
import { NOW_PLAYING_CHANGED_EVENT, NOW_PLAYING_ERROR_EVENT } from '../types/now-playing.types';

type NowPlayingStore = {
  /** Current track per bundle identifier; absent means nothing is playing. */
  tracks: Record<string, NowPlayingTrack>;
  /** Last failure per bundle identifier, most often denied Automation consent. */
  errors: Record<string, string>;
  /**
   * Applications that have reported a track at least once this session.
   *
   * Whether an application publishes anything readable cannot be known before
   * it does: Serato captures audio like any other source but never announces
   * what it is playing, and a card that says "Nothing playing" over a deck in
   * full flow is simply wrong. So a source earns its readout by producing one.
   */
  reportedBundleIds: string[];
  subscribed: boolean;

  subscribe: () => Promise<void>;
};

export const useNowPlayingStore = create<NowPlayingStore>((set, get) => ({
  tracks: {},
  errors: {},
  reportedBundleIds: [],
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

        // Reporting a track is what proves a source has metadata to show, so
        // the claim is only ever added and never taken back on a stop.
        const reportedBundleIds =
          payload.track && !state.reportedBundleIds.includes(payload.bundleId)
            ? [...state.reportedBundleIds, payload.bundleId]
            : state.reportedBundleIds;

        return { tracks, errors, reportedBundleIds };
      });
    });

    await listen<NowPlayingErrorEvent>(NOW_PLAYING_ERROR_EVENT, ({ payload }) => {
      set((state) => ({
        errors: { ...state.errors, [payload.bundleId]: payload.message },
      }));
    });

    try {
      await invoke('start_now_playing_watch');
    } catch (error) {
      console.error('Failed to start now-playing watch:', error);
    }
  },
}));
