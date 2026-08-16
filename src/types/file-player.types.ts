// A queue of audio files that plays into the mixer as a source.
//
// The motivating case is ads: a player is patched into a channel like any other
// input, so the bus routing decides where it lands — reaching the broadcast
// without reaching the tape is the thing it exists for.

import type { ConfiguredAudioDevice } from './db/configured-audio-devices.types';
import type { FilePath, Identifier, Timestamp, Uuid } from './util.types';

/** What the player is doing right now. */
export const PlaybackState = ['stopped', 'playing', 'paused'] as const;
export type PlaybackState = (typeof PlaybackState)[number];

/** What happens when the queue runs out, or a track does. */
export const RepeatMode = ['none', 'track', 'queue'] as const;
export type RepeatMode = (typeof RepeatMode)[number];

/** One file waiting to play. */
export type QueuedTrack = {
  id: Uuid<QueuedTrack>;
  filePath: FilePath;
  /**
   * What the file's tags say, when it carries any.
   *
   * Absent rather than invented for a file with no tags, which is what
   * `trackTitle` falls back on the file name for.
   */
  title: string | null;
  artist: string | null;
  album: string | null;
  /** Milliseconds, or null for a file that does not declare its length. */
  duration: number | null;
  fileSize: number;
  addedAt: Timestamp;
};

export type PlaybackMode = {
  repeatMode: RepeatMode;
  shuffle: boolean;
  crossfadeDuration: number;
};

export type PlaybackStatus = {
  state: PlaybackState;
  currentTrack: QueuedTrack | null;
  /** Where in the queue the current track sits, for marking the row. */
  currentIndex: number | null;
  /** Milliseconds into the current track. */
  position: number;
  volume: number;
  queueLength: number;
  /** The track playback pauses after, when one has been asked for. */
  breakpointTrackId: Uuid<QueuedTrack> | null;
  mode: PlaybackMode;
};

/** One press on the transport. */
export type PlaybackAction =
  | { type: 'play' }
  | { type: 'pause' }
  | { type: 'stop' }
  | { type: 'skipNext' }
  | { type: 'skipPrevious' }
  | { type: 'restartTrack' }
  | { type: 'playTrack'; trackId: Uuid<QueuedTrack> }
  | { type: 'seek'; seconds: number }
  | { type: 'setVolume'; volume: number };

/** What a new player is created with. */
export type FilePlayerConfig = {
  name: string;
  sampleRate: number;
  channels: number;
  autoPlayNext: boolean;
  volume: number;
};

/**
 * A player and the name it was created under.
 *
 * Its id is both the row's key and the identifier a channel is patched to it by
 * — the backend keys its running players by the same string it stored them
 * under, so a saved patch still resolves after a restart.
 */
export type FilePlayer = {
  id: Uuid<FilePlayer>;
  name: string;
};

/**
 * The one place a path from the operating system becomes a queueable file.
 *
 * Paths reach the interface as plain strings from the window's drag-and-drop
 * events. Branding is erased at runtime, so crossing that gap needs one
 * assertion — kept here, named, rather than spread across the drop handlers.
 */
export const asFilePath = (path: string): FilePath => path as FilePath;

/** The rate and layout a new player emits, whatever its files are recorded at. */
export const PLAYER_SAMPLE_RATE = 48000;
export const PLAYER_CHANNELS = 2;

/**
 * What the decoder can open. Matches `get_supported_audio_formats`.
 *
 * Checked before a file is queued rather than after: a file that cannot be
 * decoded would otherwise sit in the queue looking playable until it was
 * reached, which during an ad break is the worst moment to find out.
 */
export const SUPPORTED_AUDIO_EXTENSIONS = [
  'mp3',
  'flac',
  'wav',
  'ogg',
  'm4a',
  'aac',
] as const;

export const isSupportedAudioFile = (path: string): boolean => {
  const extension = path.split('.').pop()?.toLowerCase();
  return extension !== undefined && SUPPORTED_AUDIO_EXTENSIONS.includes(extension as never);
};

/**
 * Which of these players a channel is patched to, if any.
 *
 * A player is patched to by its own id — there is no prefix to recognise it by
 * the way an application tap has `app-`, because the identifier is the player's
 * row key. So the answer comes from the list rather than from the string.
 */
export const patchedPlayerId = (
  deviceIdentifier: Identifier<ConfiguredAudioDevice> | null | undefined,
  players: FilePlayer[]
): Uuid<FilePlayer> | null => {
  if (!deviceIdentifier) {
    return null;
  }

  // Compared as plain strings: the two are the same value wearing different
  // brands, which is the whole point of a player's key being its identifier.
  const match = players.find((player) => String(player.id) === String(deviceIdentifier));
  return match?.id ?? null;
};

/**
 * What to call a track.
 *
 * A file with no title tag still has to read as something, and its own name is
 * what the person who queued it recognises. The extension goes, since it says
 * nothing about the track.
 */
export const trackTitle = (track: QueuedTrack): string => {
  if (track.title) {
    return track.title;
  }

  const name = track.filePath.split('/').pop() ?? track.filePath;
  return name.replace(/\.[a-z0-9]+$/i, '');
};

/** How long a queue runs, in milliseconds, ignoring anything with no length. */
export const queueDuration = (tracks: QueuedTrack[]): number =>
  tracks.reduce((total, track) => total + (track.duration ?? 0), 0);
