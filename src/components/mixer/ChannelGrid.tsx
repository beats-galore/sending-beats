// Grid layout for channel strips
import { memo } from 'react';
import { Stack, Title, Paper, Text, Center } from '@mantine/core';
import { AudioChannel, AudioDeviceInfo } from '../../types';
import { ChannelStrip } from '../channel';

type ChannelGridProps = {
  channels: AudioChannel[];
  inputDevices: AudioDeviceInfo[];
  onRefreshDevices: () => void;
};

export const ChannelGrid = memo<ChannelGridProps>(({
  channels,
  inputDevices,
  onRefreshDevices
}) => {
  if (channels.length === 0) {
    return (
      <Paper p="lg" withBorder>
        <Center>
          <Stack align="center">
            <Text c="dimmed">No channels configured</Text>
            <Text size="sm" c="dimmed">Click "Add Channel" to get started</Text>
          </Stack>
        </Center>
      </Paper>
    );
  }

  return (
    <Stack>
      <Title order={3} c="blue">Channel Strips</Title>
      <Stack gap="md">
        {channels.map(channel => (
          <ChannelStrip
            key={channel.id}
            channel={channel}
            inputDevices={inputDevices}
            onRefreshDevices={onRefreshDevices}
          />
        ))}
      </Stack>
    </Stack>
  );
});