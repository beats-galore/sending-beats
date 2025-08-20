import {
  Stack,
  Title,
  Card,
  Grid,
  Group,
  Badge,
  Box,
  Text,
  ThemeIcon,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconClock, IconUsers } from '@tabler/icons-react';
import { memo } from 'react';
import { StatCard } from './StatCard';

type AnalyticsData = {
  currentListeners: number;
  peakListeners: number;
  totalStreamTime: string;
  topTracks: { title: string; artist: string; plays: number }[];
};

type AnalyticsTabProps = {
  analytics: AnalyticsData;
};

const useStyles = createStyles((theme) => ({
  tableCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  topTrackCard: {
    backgroundColor: theme.colors.dark[7],
    border: `1px solid ${theme.colors.dark[5]}`,
    padding: theme.spacing.md,
    borderRadius: theme.radius.md,
  },

  trackRanking: {
    borderRadius: '50%',
  },
}));

export const AnalyticsTab = memo<AnalyticsTabProps>(({ analytics }) => {
  const { classes } = useStyles();

  return (
    <Stack gap="xl">
      <Title order={3} c="blue.4">
        Analytics
      </Title>

      {/* Top Tracks */}
      <Card className={classes.tableCard} padding="lg" withBorder>
        <Title order={4} c="blue.4" mb="lg">
          Top Tracks
        </Title>
        <Stack gap="md">
          {analytics.topTracks.map((track, index) => (
            <Card key={index} className={classes.topTrackCard} withBorder>
              <Group justify="space-between" align="center">
                <Group gap="md">
                  <ThemeIcon
                    size={32}
                    color="blue"
                    variant="light"
                    className={classes.trackRanking}
                  >
                    <Text size="sm" fw={700}>
                      {index + 1}
                    </Text>
                  </ThemeIcon>
                  <Box>
                    <Text fw={600}>{track.title}</Text>
                    <Text size="sm" c="dimmed">{track.artist}</Text>
                  </Box>
                </Group>
                <Badge color="blue" variant="light">
                  {track.plays} plays
                </Badge>
              </Group>
            </Card>
          ))}
        </Stack>
      </Card>

      {/* Stream Stats */}
      <Card className={classes.tableCard} padding="lg" withBorder>
        <Title order={4} c="blue.4" mb="lg">
          Stream Statistics
        </Title>
        <Grid>
          <Grid.Col span={12} md={6}>
            <StatCard
              icon={<IconClock size={24} />}
              value={analytics.totalStreamTime}
              label="Total Stream Time"
              color="blue"
            />
          </Grid.Col>
          <Grid.Col span={12} md={6}>
            <StatCard
              icon={<IconUsers size={24} />}
              value={analytics.peakListeners}
              label="Peak Listeners"
              color="orange"
            />
          </Grid.Col>
        </Grid>
      </Card>
    </Stack>
  );
});

AnalyticsTab.displayName = 'AnalyticsTab';