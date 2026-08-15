// UI state for the studio shell.
//
// Everything here is presentation state or session bookkeeping that the audio
// engine does not own: which view is showing, what is selected, and the running
// track log. Audio state itself lives in `mixer-store` and the effects stores.
import { create } from 'zustand';
import { persist, subscribeWithSelector } from 'zustand/middleware';

export const StudioView = ['patch', 'tape', 'cast', 'devices', 'setup'] as const;
export type StudioView = (typeof StudioView)[number];

export const DestinationRole = ['MAIN', 'CUE', 'SEND'] as const;
export type DestinationRole = (typeof DestinationRole)[number];

/**
 * The node the patchbay has focused, which opens in place to reveal its
 * details. Only one node is focused at a time, so this names which kind it is
 * rather than keeping a selection per column.
 */
export type StudioSelection =
  | { kind: 'channel'; channelId: number }
  | { kind: 'cast' }
  | { kind: 'tape' }
  | { kind: 'output'; deviceId: string };

export type StreamSettings = {
  host: string;
  port: number;
  mount: string;
  username: string;
  password: string;
  bitrate: number;
  variableBitrate: boolean;
  vbrQuality: number;
};

export type LaunchSettings = {
  autoStartEngine: boolean;
  restoreLastPatch: boolean;
};

export type LoggedTrack = {
  title: string;
  artist: string;
  /** Seconds into the session at which the track started. */
  startedAt: number;
  durationSeconds: number;
};

type StudioStore = {
  view: StudioView;
  setView: (view: StudioView) => void;

  selection: StudioSelection | null;
  select: (selection: StudioSelection) => void;
  /** Closes whatever node is open, for a click that lands outside all of them. */
  clearSelection: () => void;

  drawerOpen: boolean;
  toggleDrawer: () => void;

  /**
   * Per-destination role and trim.
   *
   * The engine currently drives a single master output, so these are carried in
   * the interface rather than the pipeline. They are keyed by device identifier
   * and are not yet applied to audio.
   */
  outputRoles: Record<string, DestinationRole>;
  outputGains: Record<string, number>;
  cycleOutputRole: (deviceId: string) => void;
  setOutputGain: (deviceId: string, gainDb: number) => void;

  /** The track currently on air, and everything played before it this session. */
  nowPlayingTitle: string;
  nowPlayingArtist: string;
  setNowPlaying: (field: 'title' | 'artist', value: string) => void;
  trackLog: LoggedTrack[];
  currentTrackStartedAt: number;
  logCurrentTrack: (sessionSeconds: number) => void;

  metadataPushed: boolean;
  markMetadataPushed: (pushed: boolean) => void;

  /** Icecast target. Persisted so a restart comes back ready to go live. */
  stream: StreamSettings;
  setStream: (settings: Partial<StreamSettings>) => void;

  launch: LaunchSettings;
  toggleLaunch: (setting: keyof LaunchSettings) => void;
};

/** The slice written to local storage — durable preferences only. */
type PersistedStudioState = Pick<StudioStore, 'stream' | 'launch' | 'outputRoles' | 'outputGains'>;

const DEFAULT_STREAM: StreamSettings = {
  host: 'localhost',
  port: 8000,
  mount: '/live',
  username: 'source',
  password: '',
  bitrate: 192,
  variableBitrate: false,
  vbrQuality: 2,
};

const nextRole = (role: DestinationRole): DestinationRole =>
  DestinationRole[(DestinationRole.indexOf(role) + 1) % DestinationRole.length];

export const useStudioStore = create<StudioStore>()(
  subscribeWithSelector(
    persist<StudioStore, [], [], PersistedStudioState>(
      (set) => ({
        view: 'patch',
        setView: (view) => set({ view }),

        selection: null,
        select: (selection) => set({ selection }),
        clearSelection: () => set({ selection: null }),

        drawerOpen: true,
        toggleDrawer: () => set((state) => ({ drawerOpen: !state.drawerOpen })),

        outputRoles: {},
        outputGains: {},
        cycleOutputRole: (deviceId) =>
          set((state) => ({
            outputRoles: {
              ...state.outputRoles,
              [deviceId]: nextRole(state.outputRoles[deviceId] ?? 'MAIN'),
            },
          })),
        setOutputGain: (deviceId, gainDb) =>
          set((state) => ({
            outputGains: { ...state.outputGains, [deviceId]: Math.round(gainDb * 10) / 10 },
          })),

        nowPlayingTitle: '',
        nowPlayingArtist: '',
        setNowPlaying: (field, value) =>
          set(field === 'title' ? { nowPlayingTitle: value } : { nowPlayingArtist: value }),

        trackLog: [],
        currentTrackStartedAt: 0,
        logCurrentTrack: (sessionSeconds) =>
          set((state) => ({
            trackLog: state.nowPlayingTitle
              ? [
                  ...state.trackLog,
                  {
                    title: state.nowPlayingTitle,
                    artist: state.nowPlayingArtist,
                    startedAt: state.currentTrackStartedAt,
                    durationSeconds: Math.max(1, sessionSeconds - state.currentTrackStartedAt),
                  },
                ]
              : state.trackLog,
            currentTrackStartedAt: sessionSeconds,
            nowPlayingTitle: '',
            metadataPushed: false,
          })),

        metadataPushed: false,
        markMetadataPushed: (metadataPushed) => set({ metadataPushed }),

        stream: DEFAULT_STREAM,
        setStream: (settings) => set((state) => ({ stream: { ...state.stream, ...settings } })),

        launch: { autoStartEngine: true, restoreLastPatch: true },
        toggleLaunch: (setting) =>
          set((state) => ({ launch: { ...state.launch, [setting]: !state.launch[setting] } })),
      }),
      {
        name: 'sendin-beats-studio',
        // Only durable preferences are stored. The stream password is deliberately
        // left out — it would otherwise sit in plain text in local storage.
        partialize: (state) => ({
          stream: { ...state.stream, password: '' },
          launch: state.launch,
          outputRoles: state.outputRoles,
          outputGains: state.outputGains,
        }),
      }
    )
  )
);
