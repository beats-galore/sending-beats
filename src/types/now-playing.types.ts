// Track metadata read from captured applications.
//
// Application audio capture carries PCM only, so the backend reads track
// details separately over AppleScript and pushes them across as events.

const PlayerState = ['playing', 'paused', 'stopped'] as const;
type PlayerState = (typeof PlayerState)[number];

export type NowPlayingTrack = {
  bundleId: string;
  title: string;
  artist: string;
  album: string;
  durationSeconds: number;
  /** Where the playhead was at `positionTakenAtMs`, not where it is now. */
  positionSeconds: number;
  /**
   * Epoch milliseconds at which `positionSeconds` was true. The system session
   * republishes the playhead only when playback changes, so a reader has to age
   * it rather than take it at face value.
   */
  positionTakenAtMs: number;
  /** Zero while paused, one at normal speed. */
  playbackRate: number;
  playerState: PlayerState;
  /** Spotify only; Apple Music exposes artwork as image data rather than a URL. */
  artworkUrl: string | null;
  trackId: string;
};

export type NowPlayingEvent = {
  bundleId: string;
  /** Null once the player stops, quits, or leaves the input configuration. */
  track: NowPlayingTrack | null;
};

export type NowPlayingErrorEvent = {
  bundleId: string;
  message: string;
};

export const NOW_PLAYING_CHANGED_EVENT = 'now-playing-changed';
export const NOW_PLAYING_ERROR_EVENT = 'now-playing-error';

/**
 * Where the playhead actually is now, aged from the snapshot the player last
 * published. Clamped to the track length so a stale reading cannot run past the
 * end of it.
 */
export const trackPosition = (track: NowPlayingTrack, nowMs: number): number => {
  const elapsedSince = Math.max(0, (nowMs - track.positionTakenAtMs) / 1000);
  const position = track.positionSeconds + elapsedSince * track.playbackRate;

  return track.durationSeconds > 0 ? Math.min(position, track.durationSeconds) : position;
};

/** The prefix the mixer stores application audio sources under. */
const APPLICATION_SOURCE_PREFIX = 'app-';

/**
 * The bundle identifier behind a channel's source, or null when the channel is
 * patched to hardware rather than an application.
 */
export const bundleIdFromDeviceIdentifier = (deviceIdentifier: string): string | null =>
  deviceIdentifier.startsWith(APPLICATION_SOURCE_PREFIX)
    ? deviceIdentifier.slice(APPLICATION_SOURCE_PREFIX.length)
    : null;
