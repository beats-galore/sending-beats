import {
  Card,
  Title,
  Grid,
  Stack,
  Text,
  Badge,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

type StreamStatus = {
  is_connected: boolean;
  is_streaming: boolean;
  current_listeners: number;
  peak_listeners: number;
  stream_duration: number;
  bitrate: number;
  error_message?: string;
};

type StreamInfoCardProps = {
  streamUrl: string;
  streamStatus: StreamStatus | null;
};

const useStyles = createStyles((theme) => ({
  infoCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  streamUrl: {
    fontFamily: 'monospace',
    color: theme.colors.blue[4],
    wordBreak: 'break-all',
  },
}));

export const StreamInfoCard = memo<StreamInfoCardProps>(({ streamUrl, streamStatus }) => {
  const { classes } = useStyles();

  return (
    <Card className={classes.infoCard} padding="lg" withBorder>
      <Title order={3} c="blue.4" mb="lg">
        Stream Information
      </Title>
      <Grid>
        <Grid.Col span={12} md={6}>
          <Stack gap={4}>
            <Text size="sm" c="dimmed">Stream URL:</Text>
            <Text size="sm" className={classes.streamUrl}>
              {streamUrl}
            </Text>
          </Stack>
        </Grid.Col>
        <Grid.Col span={12} md={6}>
          <Stack gap={4}>
            <Text size="sm" c="dimmed">Status:</Text>
            <Badge
              color={streamStatus?.is_streaming ? 'green' : 'gray'}
              variant="light"
            >
              {streamStatus?.is_streaming ? 'Live' : 'Offline'}
            </Badge>
          </Stack>
        </Grid.Col>
        <Grid.Col span={12} md={6}>
          <Stack gap={4}>
            <Text size="sm" c="dimmed">Quality:</Text>
            <Text size="sm" fw={500}>
              {streamStatus?.bitrate || 128} kbps
            </Text>
          </Stack>
        </Grid.Col>
        <Grid.Col span={12} md={6}>
          <Stack gap={4}>
            <Text size="sm" c="dimmed">Listeners:</Text>
            <Text size="sm" fw={500}>
              {streamStatus?.current_listeners || 0} / {streamStatus?.peak_listeners || 0}
            </Text>
          </Stack>
        </Grid.Col>
      </Grid>
    </Card>
  );
});

StreamInfoCard.displayName = 'StreamInfoCard';