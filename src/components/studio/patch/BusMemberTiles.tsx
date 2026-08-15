import { Group, Text } from '@mantine/core';

import {
  channelTargetKey,
  outputTargetKey,
  STREAM_TARGET_KEY,
  TAPE_TARGET_KEY,
} from '../../../services/patch-color-service';
import { color } from '../../../theme/tokens';
import type { Bus } from '../../../types/bus.types';
import { useChannelDevices } from '../hooks/use-channel-devices';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { SectionLabel } from '../primitives/SectionLabel';
import { resolveDestination } from './destination-target';
import { MemberTile } from './MemberTile';

type BusMemberTilesProps = {
  bus: Bus;
};

/** What feeds a bus and what takes it, each in the colour of the card it names. */
export const BusMemberTiles = ({ bus }: BusMemberTilesProps) => {
  const channelDevices = useChannelDevices();
  const { outputs } = usePatchOutputs();

  // Membership is by device identifier. A member the interface cannot place —
  // an input whose channel was removed, or a device that has gone away — is
  // dropped rather than drawn as an unnamed tile.
  const inputs = bus.inputs.flatMap((deviceId) => {
    const channel = channelDevices.find((candidate) => candidate.deviceIdentifier === deviceId);
    return channel ? [channel] : [];
  });

  const takenBy = bus.outputs.flatMap((deviceId) => {
    const target = resolveDestination(
      deviceId,
      outputs.map((candidate) => candidate.id)
    );
    if (!target) {
      return [];
    }

    switch (target.kind) {
      case 'output':
        return [
          {
            key: deviceId,
            targetKey: outputTargetKey(deviceId),
            position: target.index,
            label: outputs[target.index].name,
          },
        ];
      // The stream and the tape carry reserved colours and no number, so they
      // are placed after the hardware outputs rather than numbered among them.
      case 'cast':
        return [
          {
            key: deviceId,
            targetKey: STREAM_TARGET_KEY,
            position: outputs.length,
            label: 'Stream',
          },
        ];
      case 'tape':
        return [
          {
            key: deviceId,
            targetKey: TAPE_TARGET_KEY,
            position: outputs.length,
            label: 'Tape',
          },
        ];
    }
  });

  return (
    <>
      <Group gap="3xs" wrap="nowrap" style={{ overflowX: 'auto', minWidth: 0 }}>
        <SectionLabel tracking="tight">FROM</SectionLabel>
        {inputs.length === 0 ? (
          <Text size="3xs" c={color.textFaintest}>
            nothing
          </Text>
        ) : (
          inputs.map((channel) => (
            <MemberTile
              key={channel.channelId}
              targetKey={channelTargetKey(channel.channelId)}
              position={channel.index}
              label={channel.name}
            />
          ))
        )}
      </Group>

      <Group gap="3xs" wrap="nowrap" style={{ overflowX: 'auto', minWidth: 0 }}>
        <SectionLabel tracking="tight">TO</SectionLabel>
        {takenBy.length === 0 ? (
          <Text size="3xs" c={color.textFaintest}>
            nothing
          </Text>
        ) : (
          takenBy.map((member) => (
            <MemberTile
              key={member.key}
              targetKey={member.targetKey}
              position={member.position}
              label={member.label}
            />
          ))
        )}
      </Group>
    </>
  );
};
