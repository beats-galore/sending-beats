//  Admin Panel - Modernized with Mantine
import { Box, Group, Title, Badge, Tabs } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconDashboard,
  IconCalendar,
  IconMusic,
  IconChartBar,
  IconWifi,
  IconWifiOff,
} from '@tabler/icons-react';
import { memo, useState, useCallback } from 'react';

import { DashboardTab, ScheduleTab, UploadsTab, AnalyticsTab } from './admin';

type ScheduleItem = {
  id: string;
  title: string;
  dj: string;
  startTime: string;
  endTime: string;
  day: string;
  isActive: boolean;
};

type AnalyticsData = {
  currentListeners: number;
  peakListeners: number;
  totalStreamTime: string;
  topTracks: { title: string; artist: string; plays: number }[];
};

type UploadedTrack = {
  id: string;
  title: string;
  artist: string;
  album: string;
  duration: string;
  fileSize: string;
  uploadDate: string;
  status: 'processing' | 'ready' | 'error';
};

const useStyles = createStyles((theme) => ({
  container: {
    padding: theme.spacing.md,
    maxWidth: 1400,
    margin: '0 auto',
  },

  statusIndicator: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },
}));

const AdminPanel = memo(() => {
  const { classes } = useStyles();
  const [activeTab, setActiveTab] = useState<'dashboard' | 'schedule' | 'uploads' | 'analytics'>(
    'dashboard'
  );
  const [isLive, setIsLive] = useState(false);
  const [currentDJ] = useState('DJ Luna');
  const [schedules] = useState<ScheduleItem[]>([
    {
      id: '1',
      title: 'Morning Mix',
      dj: 'DJ Luna',
      startTime: '08:00',
      endTime: '10:00',
      day: 'Monday',
      isActive: true,
    },
    {
      id: '2',
      title: 'Afternoon Vibes',
      dj: 'DJ Max',
      startTime: '14:00',
      endTime: '16:00',
      day: 'Wednesday',
      isActive: true,
    },
  ]);

  const [analytics] = useState<AnalyticsData>({
    currentListeners: 127,
    peakListeners: 342,
    totalStreamTime: '1,247 hours',
    topTracks: [
      { title: 'Midnight Groove', artist: 'Luna & The Stars', plays: 156 },
      { title: 'Electric Dreams', artist: 'Neon Pulse', plays: 134 },
      { title: 'Ocean Waves', artist: 'Chill Collective', plays: 98 },
    ],
  });

  const [uploads] = useState<UploadedTrack[]>([
    {
      id: '1',
      title: 'Summer Nights',
      artist: 'Chill Collective',
      album: 'Ocean Waves',
      duration: '3:45',
      fileSize: '8.2 MB',
      uploadDate: '2024-01-15',
      status: 'ready',
    },
    {
      id: '2',
      title: 'Neon City',
      artist: 'Electric Dreams',
      album: 'Cyberpunk Vibes',
      duration: '4:12',
      fileSize: '9.1 MB',
      uploadDate: '2024-01-14',
      status: 'processing',
    },
  ]);

  const handleGoLive = useCallback(() => {
    setIsLive(!isLive);
  }, [isLive]);

  const handleTabChange = useCallback((tab: string) => {
    setActiveTab(tab as 'dashboard' | 'schedule' | 'uploads' | 'analytics');
  }, []);

  return (
    <Box className={classes.container}>
      <Group justify="space-between" align="center" mb="xl">
        <Title order={1} c="blue.4">
          Admin Panel
        </Title>
        <Group className={classes.statusIndicator}>
          {isLive ? (
            <IconWifi size={20} color="#51cf66" />
          ) : (
            <IconWifiOff size={20} color="#fa5252" />
          )}
          <Badge color={isLive ? 'green' : 'gray'} variant="light" size="md">
            {isLive ? 'Live' : 'Offline'}
          </Badge>
        </Group>
      </Group>

      <Tabs value={activeTab} onChange={(value) => setActiveTab(value as any)} variant="pills">
        <Tabs.List mb="xl">
          <Tabs.Tab value="dashboard" leftSection={<IconDashboard size={16} />}>
            Dashboard
          </Tabs.Tab>
          <Tabs.Tab value="schedule" leftSection={<IconCalendar size={16} />}>
            Schedule
          </Tabs.Tab>
          <Tabs.Tab value="uploads" leftSection={<IconMusic size={16} />}>
            Uploads
          </Tabs.Tab>
          <Tabs.Tab value="analytics" leftSection={<IconChartBar size={16} />}>
            Analytics
          </Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="dashboard">
          <DashboardTab
            isLive={isLive}
            currentDJ={currentDJ}
            analytics={analytics}
            onGoLive={handleGoLive}
            onTabChange={handleTabChange}
          />
        </Tabs.Panel>
        <Tabs.Panel value="schedule">
          <ScheduleTab schedules={schedules} />
        </Tabs.Panel>
        <Tabs.Panel value="uploads">
          <UploadsTab uploads={uploads} />
        </Tabs.Panel>
        <Tabs.Panel value="analytics">
          <AnalyticsTab analytics={analytics} />
        </Tabs.Panel>
      </Tabs>
    </Box>
  );
});

AdminPanel.displayName = 'AdminPanel';

export default AdminPanel;
