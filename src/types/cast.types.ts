/** Audio formats a broadcast can be encoded as. */
export const CastFormat = ['mp3'] as const;
export type CastFormat = (typeof CastFormat)[number];

export const CAST_BITRATES = [96, 128, 192, 256, 320] as const;

/**
 * How a station is broadcast to.
 *
 * Not two settings on one mechanism. Icecast opens a connection and writes into
 * it for the length of the show; Impulse holds nothing open and sends each few
 * seconds of audio as its own request. They need different details because they
 * address a station in genuinely different terms.
 */
export const CastProtocol = ['icecast', 'impulse'] as const;
export type CastProtocol = (typeof CastProtocol)[number];

export const isCastProtocol = (value: string): value is CastProtocol =>
  CastProtocol.includes(value as CastProtocol);

export const CAST_PROTOCOL_LABELS: Record<CastProtocol, string> = {
  icecast: 'Icecast',
  impulse: 'Impulse',
};

/** Segment lengths Impulse offers, in milliseconds. */
export const CAST_SEGMENT_LENGTHS = [2000, 4000, 6000, 10000] as const;

/**
 * Somewhere this studio broadcasts to.
 *
 * Stored once and chosen, rather than typed in before each show. Global rather
 * than part of a patch: the same station is streamed to from whichever patch is
 * loaded.
 *
 * No secret. It is held in the keychain against this id, and the backend only
 * ever reports whether one is set — a source password for Icecast, an ingest
 * token for Impulse.
 */
export type CastConfiguration = {
  id: string;
  name: string;
  protocol: string;
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
  /** Impulse only. Where the ingest worker answers, scheme included. */
  endpointUrl: string | null;
  /** Impulse only. Names the station on the other end. */
  stationSlug: string | null;
  /** Impulse only. How much audio goes in one segment, and so the latency. */
  segmentMs: number;
  createdAt: string;
  updatedAt: string;
  /** Whether a secret is in the keychain, without saying what it is */
  hasPassword: boolean;
};

/** The editable half of a station: everything but its id, stamps and secret. */
export type CastConfigurationInput = Omit<
  CastConfiguration,
  'id' | 'createdAt' | 'updatedAt' | 'hasPassword'
>;

export const DEFAULT_CAST_INPUT: CastConfigurationInput = {
  name: 'New station',
  protocol: 'icecast',
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
  endpointUrl: null,
  stationSlug: null,
  segmentMs: 4000,
};

/** The editable fields of an existing station, for handing back to an update. */
export const toInput = (configuration: CastConfiguration): CastConfigurationInput => ({
  name: configuration.name,
  protocol: configuration.protocol,
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
  endpointUrl: configuration.endpointUrl,
  stationSlug: configuration.stationSlug,
  segmentMs: configuration.segmentMs,
});

/**
 * How a station reads in a list.
 *
 * The two protocols have nothing in common to show: one is a host and a mount,
 * the other an origin and a slug. Showing the Icecast fields for an Impulse
 * station would print `localhost:8000/live` under a station that broadcasts
 * nowhere near it.
 */
export const castAddress = (configuration: CastConfiguration): string => {
  if (configuration.protocol === 'impulse') {
    const endpoint = configuration.endpointUrl?.replace(/^https?:\/\//, '') ?? 'no endpoint';
    return `${endpoint}/${configuration.stationSlug ?? '…'}`;
  }

  return `${configuration.serverHost}:${configuration.serverPort}${configuration.mountPoint}`;
};

/** What the stored secret is called, which differs by protocol. */
export const castSecretLabel = (protocol: string): string =>
  protocol === 'impulse' ? 'INGEST TOKEN' : 'PASSWORD';
