import { Stack, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import type { Bus } from '../../../types/bus.types';
import { useChannelDevices } from '../hooks/use-channel-devices';
import { BusMemberRow } from './BusMemberRow';

type BusMembersProps = {
  bus: Bus;
};

/** What feeds a mix, one row per source, in the order the cables land. */
export const BusMembers = ({ bus }: BusMembersProps) => {
  const channelDevices = useChannelDevices();

  // Membership is by device identifier. A member the interface cannot place —
  // an input whose channel was removed, or a device that has gone away — is
  // dropped rather than drawn as an unnamed row.
  const inputs = bus.inputs.flatMap((deviceId) => {
    const channel = channelDevices.find((candidate) => candidate.deviceIdentifier === deviceId);
    return channel ? [channel] : [];
  });

  if (inputs.length === 0) {
    return (
      <Text size="xs" c={color.textFaintest}>
        Nothing is feeding this mix yet.
      </Text>
    );
  }

  return (
    <Stack gap={0} style={{ flex: 'none' }}>
      {inputs.map((channel) => (
        <BusMemberRow
          key={channel.channelId}
          channelId={channel.channelId}
          index={channel.index}
          name={channel.name}
        />
      ))}
    </Stack>
  );
};
