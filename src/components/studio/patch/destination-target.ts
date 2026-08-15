import { STREAM_TARGET_KEY, TAPE_TARGET_KEY } from '../../../services/patch-color-service';
import type { PatchTargetKey } from '../../../services/patch-color-service';

/**
 * The recorder registers as an ordinary mixer output while it is running, under
 * this fixed id (`isolated_audio_manager.rs`).
 */
const TAPE_OUTPUT_ID = 'recording_output';

/** Icecast registers one output per broadcast, keyed by that broadcast's id. */
const CAST_OUTPUT_PREFIX = 'icecast_output_';

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

/** What a target's colour is stored against, for the tiles naming it. */
export const targetColorKey = (
  target: DestinationTarget,
  outputTargetKeyOf: (index: number) => PatchTargetKey
): PatchTargetKey => {
  switch (target.kind) {
    case 'output':
      return outputTargetKeyOf(target.index);
    case 'cast':
      return STREAM_TARGET_KEY;
    case 'tape':
      return TAPE_TARGET_KEY;
  }
};
