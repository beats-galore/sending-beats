import { Box, ScrollArea, Stack, Title, Loader, Alert, Group, Button } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconAlertCircle, IconRefresh } from '@tabler/icons-react';
import { memo, useCallback, useEffect, useState } from 'react';

import { useApplicationAudio } from '../../hooks/use-application-audio';
import { useMixerInitialization } from '../../hooks/use-mixer-initialization';
import { useMixerRunningState } from '../../hooks/use-mixer-running-state';
import { useVUChannelStream } from '../../hooks/use-vu-channel-stream';
import { ApplicationVolumeControl } from './components/ApplicationVolumeControl';

const useStyles = createStyles((theme) => ({
  container: {
    width: '100%',
    height: '100vh',
    backgroundColor: theme.colors.dark[8],
    display: 'flex',
    flexDirection: 'column',
    padding: theme.spacing.md,
  },

  header: {
    borderBottom: `1px solid ${theme.colors.dark[5]}`,
    paddingBottom: theme.spacing.md,
    marginBottom: theme.spacing.md,
  },

  content: {
    flex: 1,
    overflow: 'hidden',
    display: 'flex',
    flexDirection: 'column',
  },

  errorContainer: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    flex: 1,
  },

  loadingContainer: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    flex: 1,
    gap: theme.spacing.md,
  },

  emptyState: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    flex: 1,
    color: theme.colors.dark[2],
  },

  scrollArea: {
    flex: 1,
  },

  appList: {
    gap: theme.spacing.sm,
  },
}));

const App = memo(() => {
  const { classes } = useStyles();
  const [autoCaptureDone, setAutoCaptureDone] = useState(false);

  // Initialize mixer
  const { isReady, error: mixerError, initialize } = useMixerInitialization();
  const isRunning = useMixerRunningState();

  // Get application audio state
  const {
    knownApps,
    activeCaptures,
    isLoading: appsLoading,
    error: appsError,
    actions,
  } = useApplicationAudio();

  // Stream VU meter data
  useVUChannelStream(isRunning);

  // Initialize mixer on mount
  useEffect(() => {
    if (!isReady && !mixerError) {
      void initialize();
    }
  }, [isReady, mixerError, initialize]);

  // Auto-capture all known applications on startup
  useEffect(() => {
    if (autoCaptureDone || !isReady || appsLoading || !knownApps.length) {
      return;
    }

    const autoCapture = async () => {
      try {
        for (const app of knownApps) {
          if (!activeCaptures.find((c) => c.pid === app.pid)) {
            await actions.startCapturing(app.pid);
          }
        }
        setAutoCaptureDone(true);
      } catch (error) {
        console.error('Failed to auto-capture applications:', error);
        setAutoCaptureDone(true);
      }
    };

    void autoCapture();
  }, [isReady, knownApps, activeCaptures, appsLoading, autoCaptureDone, actions]);

  const handleRefresh = useCallback(async () => {
    setAutoCaptureDone(false);
    await actions.refreshApplications();
  }, [actions]);

  if (mixerError) {
    return (
      <Box className={classes.container}>
        <Box className={classes.errorContainer}>
          <Alert icon={<IconAlertCircle size={16} />} title="Mixer Error" color="red">
            {mixerError}
          </Alert>
        </Box>
      </Box>
    );
  }

  if (!isReady || appsLoading) {
    return (
      <Box className={classes.container}>
        <Box className={classes.loadingContainer}>
          <Loader size="lg" />
          <div>Loading applications...</div>
        </Box>
      </Box>
    );
  }

  if (appsError) {
    return (
      <Box className={classes.container}>
        <Box className={classes.errorContainer}>
          <Stack align="center">
            <Alert icon={<IconAlertCircle size={16} />} title="Error" color="red">
              {appsError}
            </Alert>
            <Button leftSection={<IconRefresh size={16} />} onClick={handleRefresh}>
              Retry
            </Button>
          </Stack>
        </Box>
      </Box>
    );
  }

  if (!activeCaptures.length) {
    return (
      <Box className={classes.container}>
        <Box className={classes.header}>
          <Title order={3}>Volume Control</Title>
        </Box>
        <Box className={classes.emptyState}>
          <div>No applications are currently being captured</div>
        </Box>
      </Box>
    );
  }

  return (
    <Box className={classes.container}>
      <Box className={classes.header}>
        <Group justify="space-between" align="center">
          <Title order={3}>Volume Control</Title>
          <Button
            size="xs"
            variant="subtle"
            leftSection={<IconRefresh size={14} />}
            onClick={handleRefresh}
          >
            Refresh
          </Button>
        </Group>
      </Box>

      <Box className={classes.content}>
        <ScrollArea className={classes.scrollArea}>
          <Stack className={classes.appList} gap="xs">
            {activeCaptures.map((app) => (
              <ApplicationVolumeControl
                key={app.pid}
                app={app}
                onStop={() => actions.stopCapturing(app.pid)}
              />
            ))}
          </Stack>
        </ScrollArea>
      </Box>
    </Box>
  );
});

App.displayName = 'VolumeControlApp';

export default App;
