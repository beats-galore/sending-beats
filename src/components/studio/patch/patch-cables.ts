// The wiring between nodes.
//
// One cable per membership rather than per card: a source feeding two buses
// draws two cables, which is what makes a split visible on the canvas.
//
// Every end point is read off the resolved rects rather than recomputed, so a
// node the user dragged and the wire leaving it cannot disagree about where its
// port is.
import { layout } from '../../../theme/layout';
import type { Bus } from '../../../types/bus.types';
import type { ChannelDevice } from '../hooks/use-channel-devices';
import type { PatchOutput } from '../hooks/use-patch-outputs';
import type { Cable } from './CableLayer';
import { resolveDestination } from './destination-target';
import { busPortOffset, cablePath, channelPortOffset } from './patch-geometry';
import { leftPort, rightPort } from './patch-layout';
import type { NodeRect, Port } from './patch-layout';
import type { PatchRects } from './patch-rects';

const { destination } = layout;

type CableInput = {
  buses: Bus[];
  channelDevices: ChannelDevice[];
  outputs: PatchOutput[];
  rects: PatchRects;
};

/** Where a bus's output lands, and what colour the run reads as. */
const landing = (
  deviceId: string,
  outputs: PatchOutput[],
  rects: PatchRects
): { port: Port; tone: Cable['tone'] } | null => {
  const target = resolveDestination(
    deviceId,
    outputs.map((output) => output.id)
  );

  switch (target?.kind) {
    case 'output':
      return {
        port: leftPort(rects.outputs[target.index], destination.outputPortOffset),
        tone: outputs[target.index].role === 'CUE' ? 'warn' : 'accent',
      };
    case 'cast':
      return { port: leftPort(rects.cast, destination.castPortOffset), tone: 'hot' };
    case 'tape':
      return { port: leftPort(rects.tape, destination.tapePortOffset), tone: 'hot' };
    default:
      return null;
  }
};

const sourceCables = ({ buses, channelDevices, rects }: CableInput): Cable[] =>
  buses.flatMap((busEntry, busIndex) =>
    busEntry.inputs.flatMap((deviceId, portIndex) => {
      const channel = channelDevices.find(
        (candidate) => candidate.deviceIdentifier === deviceId
      );
      const from: NodeRect | undefined = channel && rects.channels[channel.index];
      if (!from) {
        return [];
      }

      return [
        {
          id: `in-${busEntry.id}-${deviceId}`,
          path: cablePath(
            rightPort(from, channelPortOffset),
            leftPort(rects.buses[busIndex], busPortOffset(portIndex, busEntry.inputs.length))
          ),
          tone: 'accent' as const,
          active: true,
        },
      ];
    })
  );

const outputCables = ({ buses, outputs, rects }: CableInput): Cable[] =>
  buses.flatMap((busEntry, busIndex) =>
    busEntry.outputs.flatMap((deviceId, portIndex) => {
      const target = landing(deviceId, outputs, rects);
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
          tone: target.tone,
          active: busEntry.inputs.length > 0,
        },
      ];
    })
  );

export const patchCables = (input: CableInput): Cable[] => [
  ...sourceCables(input),
  ...outputCables(input),
];
