/** Audio formats a broadcast can be encoded as. */
export const CastFormat = ['mp3'] as const;
export type CastFormat = (typeof CastFormat)[number];

export const CAST_BITRATES = [96, 128, 192, 256, 320] as const;

/**
 * Somewhere this studio broadcasts to.
 *
 * Stored once and chosen, rather than typed in before each show. Global rather
 * than part of a patch: the same station is streamed to from whichever patch is
 * loaded.
 *
 * No password. It is held in the keychain against this id, and the backend only
 * ever reports whether one is set.
 */
export type CastConfiguration = {
  id: string;
  name: string;
  serverHost: string;
  serverPort: number;
  mountPoint: string;
  username: string;
  streamName: string;
  streamDescription: string;
  streamGenre: string;
  streamUrl: string;
  isPublic: boolean;
  audioFormat: string;
  bitrateKbps: number;
  variableBitrate: boolean;
  vbrQuality: number;
  createdAt: string;
  updatedAt: string;
  /** Whether a password is in the keychain, without saying what it is */
  hasPassword: boolean;
};

/** The editable half of a station: everything but its id, stamps and password. */
export type CastConfigurationInput = Omit<
  CastConfiguration,
  'id' | 'createdAt' | 'updatedAt' | 'hasPassword'
>;

export const DEFAULT_CAST_INPUT: CastConfigurationInput = {
  name: 'New station',
  serverHost: 'localhost',
  serverPort: 8000,
  mountPoint: '/live',
  username: 'source',
  streamName: '',
  streamDescription: '',
  streamGenre: '',
  streamUrl: '',
  isPublic: false,
  audioFormat: 'mp3',
  bitrateKbps: 192,
  variableBitrate: false,
  vbrQuality: 4,
};

/** The editable fields of an existing station, for handing back to an update. */
export const toInput = (configuration: CastConfiguration): CastConfigurationInput => ({
  name: configuration.name,
  serverHost: configuration.serverHost,
  serverPort: configuration.serverPort,
  mountPoint: configuration.mountPoint,
  username: configuration.username,
  streamName: configuration.streamName,
  streamDescription: configuration.streamDescription,
  streamGenre: configuration.streamGenre,
  streamUrl: configuration.streamUrl,
  isPublic: configuration.isPublic,
  audioFormat: configuration.audioFormat,
  bitrateKbps: configuration.bitrateKbps,
  variableBitrate: configuration.variableBitrate,
  vbrQuality: configuration.vbrQuality,
});
