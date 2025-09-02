// Professional Virtual Mixer - Refactored with modern architecture
import { Box, Title, Stack, Alert, Button, Group, Grid } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconAlertCircle, IconRefresh } from '@tabler/icons-react';
import { memo, useEffect, useCallback } from 'react';

import {
  useAudioDevicesStatus,
  useMixerInitialization,
  useMixerRunningState,
  useVUMeterData,
} from '../../hooks';
import { RecordingControlsCard, RecordingConfigCard, RecordingHistoryCard } from '../dj';
import { ErrorBoundary, FullScreenLoader } from '../layout';

import { ChannelGrid } from './ChannelGrid';
import { MasterSection } from './MasterSection';
import { MixerControls } from './MixerControls';

// Import recording components

const useStyles = createStyles((theme) => ({
  container: {
    height: '100%',
    width: '100%',
    maxWidth: '100%',
    overflow: 'auto',
    padding: theme.spacing.md,
    boxSizing: 'border-box',
    display: 'flex',
    flexDirection: 'column',
  },

  mainStack: {
    width: '100%',
    maxWidth: '100%',
    overflow: 'visible',
    flex: 1,
  },

  errorContainer: {
    height: '100%',
    width: '100%',
    maxWidth: '100%',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: theme.spacing.md,
  },

  errorAlert: {
    maxWidth: 400,
  },
}));

const VirtualMixer = memo(() => {
  const { classes } = useStyles();
  console.log('re-rendered virtual mixer');

  const { error: devicesError } = useAudioDevicesStatus();

  const { isReady, error: mixerError, initialize } = useMixerInitialization();

  const isRunning = useMixerRunningState();

  // Start VU meter polling when running
  useVUMeterData(isRunning);

  const handleInitialize = useCallback(() => {
    void initialize();
  }, [initialize]);

  // Initialize mixer on mount - memoize the effect callback
  const initializeEffect = useCallback(() => {
    if (!isReady && !mixerError) {
      void initialize();
    }
  }, [isReady, mixerError, initialize]);

  useEffect(() => {
    initializeEffect();
  }, [initializeEffect]);

  if (devicesError) {
    return (
      <Box className={classes.errorContainer}>
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Device Error"
          color="red"
          className={classes.errorAlert}
        >
          {devicesError}
        </Alert>
      </Box>
    );
  }

  if (mixerError) {
    return (
      <Box className={classes.errorContainer}>
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Mixer Error"
          color="red"
          className={classes.errorAlert}
        >
          {mixerError}
          <Group mt="md">
            <Button color="red" leftSection={<IconRefresh size={16} />} onClick={handleInitialize}>
              Retry
            </Button>
          </Group>
        </Alert>
      </Box>
    );
  }

  return (
    <Box className={classes.container}>
      <Stack gap="lg" className={classes.mainStack}>
        <Title order={1} c="blue">
          Virtual Mixer
        </Title>

        {/* Mixer Controls */}
        <MixerControls />

        {/* Channel Grid */}
        {isReady && <ChannelGrid />}

        {/* Master Section */}
        {isReady && <MasterSection />}

        {/* Recording Section */}
        {isReady && (
          <Stack gap="md">
            <Grid>
              {/* Recording Controls */}
              <Grid.Col span={{ base: 12, md: 6 }}>
                <RecordingControlsCard />
              </Grid.Col>

              {/* Recording Configuration */}
              <Grid.Col span={{ base: 12, md: 6 }}>
                <RecordingConfigCard />
              </Grid.Col>
            </Grid>

            {/* Recording History */}
            <RecordingHistoryCard maxHeight={250} />
          </Stack>
        )}
      </Stack>
    </Box>
  );
});

VirtualMixer.displayName = 'VirtualMixer';

// Wrap in error boundary
export const VirtualMixerWithErrorBoundary = () => (
  <ErrorBoundary>
    <VirtualMixer />
  </ErrorBoundary>
);

export default VirtualMixer;
