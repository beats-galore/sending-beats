import { Card, Group, Text, Title, Badge, Grid, Stack, ThemeIcon } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconWifi, IconWifiOff, IconUsers, IconClock, IconBroadcast } from '@tabler/icons-react';
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

type StreamStatusCardProps = {
  streamStatus: StreamStatus | null;
};

const useStyles = createStyles((theme) => ({
  statusCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  connectionStatus: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },

  statContainer: {
    textAlign: 'center',
  },
}));

export const StreamStatusCard = memo<StreamStatusCardProps>(({ streamStatus }) => {
  const { classes } = useStyles();

  return (
    <Card className={classes.statusCard} padding="lg" withBorder>
      <Group justify="space-between" mb="md">
        <Title order={3} c="blue.4">
          Stream Status
        </Title>
        <Group className={classes.connectionStatus}>
          {streamStatus?.is_connected ? (
            <IconWifi size={16} color="#51cf66" />
          ) : (
            <IconWifiOff size={16} color="#fa5252" />
          )}
          <Badge color={streamStatus?.is_connected ? 'green' : 'red'} variant="light" size="sm">
            {streamStatus?.is_connected ? 'Connected' : 'Disconnected'}
          </Badge>
        </Group>
      </Group>

      {streamStatus && (
        <Grid>
          <Grid.Col span={{ base: 6, md: 3 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconUsers size={20} color="#339af0" />
              <Text size="xl" fw={700} c="blue.4">
                {streamStatus.current_listeners}
              </Text>
              <Text size="xs" c="dimmed">
                Current Listeners
              </Text>
            </Stack>
          </Grid.Col>

          <Grid.Col span={{ base: 6, md: 3 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconUsers size={20} color="#fd7e14" />
              <Text size="xl" fw={700} c="orange.4">
                {streamStatus.peak_listeners}
              </Text>
              <Text size="xs" c="dimmed">
                Peak Listeners
              </Text>
            </Stack>
          </Grid.Col>

          <Grid.Col span={{ base: 6, md: 3 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconClock size={20} color="#339af0" />
              <Text size="xl" fw={700} c="blue.4">
                {streamStatus.stream_duration}s
              </Text>
              <Text size="xs" c="dimmed">
                Stream Duration
              </Text>
            </Stack>
          </Grid.Col>

          <Grid.Col span={{ base: 6, md: 3 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconBroadcast size={20} color="#339af0" />
              <Text size="xl" fw={700} c="blue.4">
                {streamStatus.bitrate} kbps
              </Text>
              <Text size="xs" c="dimmed">
                Bitrate
              </Text>
            </Stack>
          </Grid.Col>
        </Grid>
      )}
    </Card>
  );
});

StreamStatusCard.displayName = 'StreamStatusCard';
