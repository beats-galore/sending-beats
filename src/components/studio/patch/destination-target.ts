
/**
 * The recorder registers as an ordinary mixer output while it is running, under
 * this fixed id (`isolated_audio_manager.rs`).
 */
const TAPE_OUTPUT_ID = 'recording_output';

/** Icecast registers one output per broadcast, keyed by the station's id. */
const CAST_OUTPUT_PREFIX = 'icecast_output_';

/**
 * What the mixer names a station's broadcast.
 *
 * Built from the station's own row key, so routing set while off air is still
 * pointing at the same output when it goes live.
 */
export const castOutputId = (castConfigurationId: string): string =>
  `${CAST_OUTPUT_PREFIX}${castConfigurationId}`;

/**
 * Where a bus member sits on the canvas.
 *
 * The stream and the tape are ordinary outputs to the mixing layer but are not
 * in the destination list, because their ids only exist while they are running
 * and cannot be configured (see issue #116). They still have to be drawn — a
 * bus feeding the broadcast that showed no connection would be worse than one
 * that cannot be edited.
 */
export type DestinationTarget =
  | { kind: 'output'; index: number }
  | { kind: 'cast' }
  | { kind: 'tape' };

export const resolveDestination = (
  deviceId: string,
  outputIds: string[]
): DestinationTarget | null => {
  const index = outputIds.indexOf(deviceId);
  if (index >= 0) {
    return { kind: 'output', index };
  }

  if (deviceId === TAPE_OUTPUT_ID) {
    return { kind: 'tape' };
  }

  if (deviceId.startsWith(CAST_OUTPUT_PREFIX)) {
    return { kind: 'cast' };
  }

  return null;
};
