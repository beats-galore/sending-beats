import { Card, Group, Text, Title, Badge, Grid, Stack, Progress, Tooltip } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconHeart, IconWifi, IconClock, IconReload, IconTrendingUp } from '@tabler/icons-react';
import { memo } from 'react';

type ConnectionDiagnostics = {
  latency_ms: number | null;
  packet_loss_rate: number;
  connection_stability: number; // 0.0 to 1.0
  reconnect_attempts: number;
  time_since_last_reconnect_seconds: number | null;
  connection_uptime_seconds: number | null;
};

type BitrateInfo = {
  current_bitrate: number;
  available_bitrates: number[];
  codec: string;
};

type AudioStreamingStats = {
  samples_processed: number;
  samples_per_second: number;
  buffer_overruns: number;
  encoding_errors: number;
};

type StreamDiagnosticsCardProps = {
  connectionDiagnostics: ConnectionDiagnostics;
  bitrateInfo: BitrateInfo;
  audioStats: AudioStreamingStats | null;
};

const useStyles = createStyles((theme) => ({
  diagnosticsCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  statContainer: {
    textAlign: 'center',
  },

  progressContainer: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },

  stabilityBadge: {
    minWidth: '60px',
    textAlign: 'center',
  },
}));

const formatDuration = (seconds: number | null): string => {
  if (!seconds) return 'N/A';

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  }
  return `${secs}s`;
};

const getStabilityColor = (stability: number): string => {
  if (stability >= 0.8) return 'green';
  if (stability >= 0.6) return 'yellow';
  return 'red';
};

const getStabilityLabel = (stability: number): string => {
  if (stability >= 0.9) return 'Excellent';
  if (stability >= 0.8) return 'Good';
  if (stability >= 0.6) return 'Fair';
  if (stability >= 0.4) return 'Poor';
  return 'Critical';
};

export const StreamDiagnosticsCard = memo<StreamDiagnosticsCardProps>(
  ({ connectionDiagnostics, bitrateInfo, audioStats }) => {
    const { classes } = useStyles();

    return (
      <Card className={classes.diagnosticsCard} padding="lg" withBorder>
        <Title order={3} c="blue.4" mb="md">
          Stream Diagnostics
        </Title>

        <Grid>
          {/* Connection Stability */}
          <Grid.Col span={{ base: 12, md: 6 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconHeart size={20} color="#51cf66" />
              <Group className={classes.progressContainer}>
                <Progress
                  value={connectionDiagnostics.connection_stability * 100}
                  color={getStabilityColor(connectionDiagnostics.connection_stability)}
                  size="lg"
                  radius="md"
                  style={{ width: '80px' }}
                />
                <Badge
                  color={getStabilityColor(connectionDiagnostics.connection_stability)}
                  variant="light"
                  size="sm"
                  className={classes.stabilityBadge}
                >
                  {getStabilityLabel(connectionDiagnostics.connection_stability)}
                </Badge>
              </Group>
              <Text size="xs" c="dimmed">
                Connection Stability
              </Text>
            </Stack>
          </Grid.Col>

          {/* Latency */}
          <Grid.Col span={{ base: 12, md: 6 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconWifi size={20} color="#339af0" />
              <Text size="xl" fw={700} c="blue.4">
                {connectionDiagnostics.latency_ms ? `${connectionDiagnostics.latency_ms}ms` : 'N/A'}
              </Text>
              <Text size="xs" c="dimmed">
                Latency
              </Text>
            </Stack>
          </Grid.Col>

          {/* Connection Uptime */}
          <Grid.Col span={{ base: 12, md: 6 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconClock size={20} color="#51cf66" />
              <Text size="xl" fw={700} c="green.4">
                {formatDuration(connectionDiagnostics.connection_uptime_seconds)}
              </Text>
              <Text size="xs" c="dimmed">
                Connection Uptime
              </Text>
            </Stack>
          </Grid.Col>

          {/* Reconnect Attempts */}
          <Grid.Col span={{ base: 12, md: 6 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconReload size={20} color="#fd7e14" />
              <Text size="xl" fw={700} c="orange.4">
                {connectionDiagnostics.reconnect_attempts}
              </Text>
              <Text size="xs" c="dimmed">
                Reconnect Attempts
              </Text>
            </Stack>
          </Grid.Col>

          {/* Bitrate Info */}
          <Grid.Col span={{ base: 12, md: 6 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <IconTrendingUp size={20} color="#9775fa" />
              <Text size="xl" fw={700} c="violet.4">
                {bitrateInfo.current_bitrate} kbps
              </Text>
              <Text size="xs" c="dimmed">
                Current Bitrate ({bitrateInfo.codec})
              </Text>
            </Stack>
          </Grid.Col>

          {/* Audio Processing Stats */}
          {audioStats && (
            <Grid.Col span={{ base: 12, md: 6 }}>
              <Stack align="center" gap={4} className={classes.statContainer}>
                <IconTrendingUp size={20} color="#339af0" />
                <Tooltip
                  label={`${audioStats.samples_processed.toLocaleString()} samples processed`}
                >
                  <Text size="xl" fw={700} c="blue.4">
                    {Math.round(audioStats.samples_per_second).toLocaleString()}/s
                  </Text>
                </Tooltip>
                <Text size="xs" c="dimmed">
                  Samples Per Second
                </Text>
              </Stack>
            </Grid.Col>
          )}

          {/* Packet Loss Rate */}
          <Grid.Col span={{ base: 12, md: 6 }}>
            <Stack align="center" gap={4} className={classes.statContainer}>
              <Text
                size="xl"
                fw={700}
                c={connectionDiagnostics.packet_loss_rate > 0.1 ? 'red.4' : 'green.4'}
              >
                {(connectionDiagnostics.packet_loss_rate * 100).toFixed(1)}%
              </Text>
              <Text size="xs" c="dimmed">
                Packet Loss
              </Text>
            </Stack>
          </Grid.Col>

          {/* Buffer Status */}
          {audioStats && (
            <Grid.Col span={{ base: 12, md: 6 }}>
              <Stack align="center" gap={4} className={classes.statContainer}>
                <Text size="xl" fw={700} c={audioStats.buffer_overruns > 0 ? 'red.4' : 'green.4'}>
                  {audioStats.buffer_overruns}
                </Text>
                <Text size="xs" c="dimmed">
                  Buffer Overruns
                </Text>
              </Stack>
            </Grid.Col>
          )}
        </Grid>
      </Card>
    );
  }
);

StreamDiagnosticsCard.displayName = 'StreamDiagnosticsCard';
