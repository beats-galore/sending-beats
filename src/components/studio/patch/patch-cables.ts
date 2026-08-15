// The wiring between nodes.
//
// One cable per membership rather than per card: a source feeding two buses
// draws two cables, which is what makes a split visible on the canvas.
//
// Every end point is read off the resolved rects rather than recomputed, so a
// node the user dragged and the wire leaving it cannot disagree about where its
// port is.
import {
  channelTargetKey,
  outputTargetKey,
  STREAM_TARGET_KEY,
  TAPE_TARGET_KEY,
} from '../../../services/patch-color-service';
import type { PatchTargetKey } from '../../../services/patch-color-service';
import { layout } from '../../../theme/layout';
import type { Bus } from '../../../types/bus.types';
import type { ChannelDevice } from '../hooks/use-channel-devices';
import type { PatchOutput } from '../hooks/use-patch-outputs';
import type { Cable } from './CableLayer';
import { resolveDestination } from './destination-target';
import {
  busInputPortOffset,
  busPortOffset,
  cablePath,
  channelPortOffset,
} from './patch-geometry';
import { leftPort, rightPort } from './patch-layout';
import type { NodeRect, Port } from './patch-layout';
import type { PatchRects } from './patch-rects';

const { destination } = layout;

type CableInput = {
  buses: Bus[];
  channelDevices: ChannelDevice[];
  outputs: PatchOutput[];
  rects: PatchRects;
  /** The colour of the thing a key names, for painting a run by what it carries. */
  colorFor: (targetKey: PatchTargetKey, position: number) => string;
};

/** Where a bus's output lands, and what colour the run reads as. */
const landing = (
  deviceId: string,
  { outputs, rects, colorFor }: CableInput
): { port: Port; color: string } | null => {
  const target = resolveDestination(
    deviceId,
    outputs.map((output) => output.id)
  );

  switch (target?.kind) {
    case 'output':
      return {
        port: leftPort(rects.outputs[target.index], destination.outputPortOffset),
        color: colorFor(outputTargetKey(outputs[target.index].id), target.index),
      };
    case 'cast':
      return {
        port: leftPort(rects.cast, destination.castPortOffset),
        color: colorFor(STREAM_TARGET_KEY, 0),
      };
    case 'tape':
      return {
        port: leftPort(rects.tape, destination.tapePortOffset),
        color: colorFor(TAPE_TARGET_KEY, 0),
      };
    default:
      return null;
  }
};

const sourceCables = (input: CableInput): Cable[] => {
  const { buses, channelDevices, rects, colorFor } = input;

  return buses.flatMap((busEntry, busIndex) =>
    busEntry.inputs.flatMap((deviceId, portIndex) => {
      const channel = channelDevices.find(
        (candidate) => candidate.deviceIdentifier === deviceId
      );
      const from: NodeRect | undefined = channel && rects.channels[channel.index];
      if (!channel || !from) {
        return [];
      }

      return [
        {
          id: `in-${busEntry.id}-${deviceId}`,
          path: cablePath(
            rightPort(from, channelPortOffset),
            leftPort(rects.buses[busIndex], busInputPortOffset(portIndex))
          ),
          color: colorFor(channelTargetKey(channel.channelId), channel.index),
          active: true,
        },
      ];
    })
  );
};

const outputCables = (input: CableInput): Cable[] => {
  const { buses, rects } = input;

  return buses.flatMap((busEntry, busIndex) =>
    busEntry.outputs.flatMap((deviceId, portIndex) => {
      const target = landing(deviceId, input);
      if (!target) {
        return [];
      }

      return [
        {
          id: `out-${busEntry.id}-${deviceId}`,
          path: cablePath(
            rightPort(
              rects.buses[busIndex],
              busPortOffset(portIndex, busEntry.outputs.length)
            ),
            target.port
          ),
          color: target.color,
          active: busEntry.inputs.length > 0,
        },
      ];
    })
  );
};

export const patchCables = (input: CableInput): Cable[] => [
  ...sourceCables(input),
  ...outputCables(input),
];
