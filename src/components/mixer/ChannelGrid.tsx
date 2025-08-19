// Grid layout for channel strips - Professional horizontal mixer layout
import { Stack, Title, Paper, Text, Center, Group, ScrollArea } from '@mantine/core';
import { memo, useMemo } from 'react';

import { ChannelStrip } from '../channel';
import { useChannelsData } from '../../hooks';

export const ChannelGrid = memo(() => {
  const { channels } = useChannelsData();
    const groupStyle = useMemo(() => ({
      minWidth: `${channels.length * 280}px`
    }), [channels.length]);
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
          <ScrollArea scrollbars="x" offsetScrollbars>
            <Group
              gap="sm"
              align="flex-start"
              wrap="nowrap"
              style={groupStyle}
            >
              {channels.map((channel) => (
                <ChannelStrip
                  key={channel.id}
                  channel={channel}
                />
              ))}
            </Group>
          </ScrollArea>
        </Paper>
      </Stack>
    );
  }
);
