// Track metadata read from captured applications.
//
// Application audio capture carries PCM only, so the backend reads track
// details separately over AppleScript and pushes them across as events.

export const PlayerState = ['playing', 'paused', 'stopped'] as const;
export type PlayerState = (typeof PlayerState)[number];

export type NowPlayingTrack = {
  bundleId: string;
  title: string;
  artist: string;
  album: string;
  durationSeconds: number;
  positionSeconds: number;
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

export type NowPlayingPlayerInfo = {
  bundleId: string;
  displayName: string;
};

export const NOW_PLAYING_CHANGED_EVENT = 'now-playing-changed';
export const NOW_PLAYING_ERROR_EVENT = 'now-playing-error';

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
