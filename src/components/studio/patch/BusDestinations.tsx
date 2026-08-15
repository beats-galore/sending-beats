import { Group, Text } from '@mantine/core';

import {
  outputTargetKey,
  STREAM_TARGET_KEY,
  TAPE_TARGET_KEY,
} from '../../../services/patch-color-service';
import type { PatchTargetKey } from '../../../services/patch-color-service';
import { usePatchColorStore } from '../../../stores/patch-color-store';
import { color } from '../../../theme/tokens';
import type { Bus } from '../../../types/bus.types';
import { patchColorOf } from '../hooks/use-patch-color';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { resolveDestination } from './destination-target';

type BusDestinationsProps = {
  bus: Bus;
};

/**
 * Where a mix goes, as one line.
 *
 * A row per destination the way the members are listed would double the height
 * of every mix to say something most of them answer in a few words. Each name
 * still carries its own colour, so it can be matched to the card it names and
 * to the cable running there.
 */
export const BusDestinations = ({ bus }: BusDestinationsProps) => {
  const { outputs } = usePatchOutputs();
  const colors = usePatchColorStore((state) => state.colors);

  const named = bus.outputs.flatMap((deviceId) => {
    const target = resolveDestination(
      deviceId,
      outputs.map((candidate) => candidate.id)
    );
    if (!target) {
      return [];
    }

    // The stream and the tape carry reserved colours and no number, so they are
    // placed after the hardware outputs rather than numbered among them.
    const entry: { key: string; targetKey: PatchTargetKey; position: number; label: string } =
      target.kind === 'output'
        ? {
            key: deviceId,
            targetKey: outputTargetKey(deviceId),
            position: target.index,
            label: outputs[target.index].name,
          }
        : target.kind === 'cast'
          ? {
              key: deviceId,
              targetKey: STREAM_TARGET_KEY,
              position: outputs.length,
              label: 'Stream',
            }
          : { key: deviceId, targetKey: TAPE_TARGET_KEY, position: outputs.length, label: 'Tape' };

    return [entry];
  });

  return (
    <Group gap="xs" wrap="nowrap" style={{ flex: 'none', overflow: 'hidden' }}>
      <Text size="2xs" c={color.textFaintest} style={{ flex: 'none' }}>
        →
      </Text>
      {named.length === 0 ? (
        <Text size="2xs" c={color.textFaintest} truncate>
          nobody is listening
        </Text>
      ) : (
        named.map((destination, index) => (
          <Group gap="xs" wrap="nowrap" key={destination.key} style={{ flex: '0 1 auto', minWidth: 0 }}>
            {index > 0 && (
              <Text size="2xs" c={color.textFaintest} style={{ flex: 'none' }}>
                ·
              </Text>
            )}
            <Text
              size="2xs"
              truncate
              c={patchColorOf(colors, destination.targetKey, destination.position).value}
              title={destination.label}
            >
              {destination.label}
            </Text>
          </Group>
        ))
      )}
    </Group>
  );
};
