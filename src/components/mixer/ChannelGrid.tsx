// Grid layout for channel strips - Horizontal rows layout
import { Stack, Title, Paper, Text, Center } from '@mantine/core';
import { memo } from 'react';

import { ChannelStrip } from '../channel';
import { useChannelsData } from '../../hooks';

export const ChannelGrid = memo(() => {
  const { channels } = useChannelsData();
  if (channels.length === 0) {
    return (
      <Paper p="lg" withBorder>
        <Center>
          <Stack align="center">
            <Text c="dimmed">No channels configured</Text>
            <Text size="sm" c="dimmed">
              {`Click "Add Channel" to get started`}
            </Text>
          </Stack>
        </Center>
      </Paper>
    );
  }

  return (
    <Stack gap="sm" w="100%">
      <Title order={3} c="blue">
        Channel Strips
      </Title>
      <Paper p="md" withBorder radius="md" w="100%">
        <Stack gap="xs">
          {channels.map((channel) => (
            <ChannelStrip key={channel.id} channel={channel} />
          ))}
        </Stack>
      </Paper>
    </Stack>
  );
});
