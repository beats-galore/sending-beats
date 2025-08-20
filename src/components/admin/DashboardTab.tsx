import {
  Stack,
  Card,
  Group,
  Title,
  Badge,
  Grid,
  Button,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconWifi,
  IconWifiOff,
  IconUsers,
  IconBroadcast,
  IconPlayerPlay,
  IconPlayerStop,
  IconCalendar,
  IconChartBar,
  IconMusic,
  IconSettings,
} from '@tabler/icons-react';
import { memo } from 'react';
import { StatCard } from './StatCard';
import { QuickActionCard } from './QuickActionCard';

type AnalyticsData = {
  currentListeners: number;
  peakListeners: number;
  totalStreamTime: string;
  topTracks: { title: string; artist: string; plays: number }[];
};

type DashboardTabProps = {
  isLive: boolean;
  currentDJ: string;
  analytics: AnalyticsData;
  onGoLive: () => void;
  onTabChange: (tab: string) => void;
};

const useStyles = createStyles((theme) => ({
  tableCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  statusIndicator: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },
}));

export const DashboardTab = memo<DashboardTabProps>(({
  isLive,
  currentDJ,
  analytics,
  onGoLive,
  onTabChange,
}) => {
  const { classes } = useStyles();

  return (
    <Stack gap="xl">
      {/* Live Status */}
      <Card className={classes.tableCard} padding="lg" withBorder>
        <Group justify="space-between" align="center" mb="lg">
          <Title order={3} c="blue.4">
            Live Status
          </Title>
          <Group className={classes.statusIndicator}>
            {isLive ? (
              <IconWifi size={16} color="#51cf66" />
            ) : (
              <IconWifiOff size={16} color="#fa5252" />
            )}
            <Badge
              color={isLive ? 'green' : 'red'}
              variant="light"
              size="md"
            >
              {isLive ? 'ON AIR' : 'OFF AIR'}
            </Badge>
          </Group>
        </Group>
        
        <Grid mb="lg">
          <Grid.Col span={12} sm={6} md={4}>
            <StatCard
              icon={<IconUsers size={20} />}
              value={analytics.currentListeners}
              label="Current Listeners"
              color="blue"
            />
          </Grid.Col>
          <Grid.Col span={12} sm={6} md={4}>
            <StatCard
              icon={<IconUsers size={20} />}
              value={analytics.peakListeners}
              label="Peak Listeners"
              color="orange"
            />
          </Grid.Col>
          <Grid.Col span={12} sm={6} md={4}>
            <StatCard
              icon={<IconBroadcast size={20} />}
              value={currentDJ}
              label="Current DJ"
              color="green"
            />
          </Grid.Col>
        </Grid>
        
        <Button
          onClick={onGoLive}
          leftSection={isLive ? <IconPlayerStop size={16} /> : <IconPlayerPlay size={16} />}
          color={isLive ? 'red' : 'green'}
          variant={isLive ? 'light' : 'filled'}
        >
          {isLive ? 'Go Off Air' : 'Go Live'}
        </Button>
      </Card>

      {/* Quick Actions */}
      <Card className={classes.tableCard} padding="lg" withBorder>
        <Title order={3} c="blue.4" mb="lg">
          Quick Actions
        </Title>
        <Grid>
          <Grid.Col span={6} md={3}>
            <QuickActionCard
              icon={<IconCalendar size={24} />}
              title="Schedule"
              description="Manage shows"
              onClick={() => onTabChange('schedule')}
            />
          </Grid.Col>
          <Grid.Col span={6} md={3}>
            <QuickActionCard
              icon={<IconChartBar size={24} />}
              title="Analytics"
              description="View stats"
              onClick={() => onTabChange('analytics')}
            />
          </Grid.Col>
          <Grid.Col span={6} md={3}>
            <QuickActionCard
              icon={<IconMusic size={24} />}
              title="Upload"
              description="Add music"
              onClick={() => onTabChange('uploads')}
            />
          </Grid.Col>
          <Grid.Col span={6} md={3}>
            <QuickActionCard
              icon={<IconSettings size={24} />}
              title="Settings"
              description="Configure"
            />
          </Grid.Col>
        </Grid>
      </Card>
    </Stack>
  );
});

DashboardTab.displayName = 'DashboardTab';