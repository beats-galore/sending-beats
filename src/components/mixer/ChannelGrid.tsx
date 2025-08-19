// Grid layout for channel strips - Professional horizontal mixer layout
import { memo } from 'react';
import { Stack, Title, Paper, Text, Center, Group, ScrollArea } from '@mantine/core';
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
    <Stack gap="sm" w="100%">
      <Title order={3} c="blue">Channel Strips</Title>
      <Paper p="md" withBorder radius="md" w="100%">
        <ScrollArea scrollbars="x" offsetScrollbars>
          <Group gap="sm" align="flex-start" wrap="nowrap" style={{ minWidth: `${channels.length * 280}px` }}>
            {channels.map(channel => (
              <ChannelStrip
                key={channel.id}
                channel={channel}
                inputDevices={inputDevices}
                onRefreshDevices={onRefreshDevices}
              />
            ))}
          </Group>
        </ScrollArea>
      </Paper>
    </Stack>
  );
});