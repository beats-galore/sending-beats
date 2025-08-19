// Professional Virtual Mixer - Refactored with modern architecture
import { Container, Title, Stack, Alert, Button, Group } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconAlertCircle, IconRefresh } from '@tabler/icons-react';
import { memo, useEffect, useCallback } from 'react';

import {
  useAudioDevicesStatus,
  useMixerInitialization,
  useMixerRunningState,
  useVUMeterData,
} from '../../hooks';
import { ErrorBoundary, FullScreenLoader } from '../layout';

import { ChannelGrid } from './ChannelGrid';
import { MasterSection } from './MasterSection';
import { MixerControls } from './MixerControls';

const useStyles = createStyles(() => ({
  container: {
    minHeight: '100vh',
    maxWidth: '100%',
  },

  errorContainer: {
    minHeight: '100vh',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
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
      <Container fluid p="md" className={classes.errorContainer}>
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Device Error"
          color="red"
          className={classes.errorAlert}
        >
          {devicesError}
        </Alert>
      </Container>
    );
  }

  if (mixerError) {
    return (
      <Container fluid p="md" className={classes.errorContainer}>
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
      </Container>
    );
  }

  return (
    <Container fluid p="md" className={classes.container}>
      <Stack gap="lg" w="100%">
        <Title order={1} c="blue">
          Virtual Mixer
        </Title>

        {/* Mixer Controls */}
        <MixerControls />

        {/* Channel Grid */}
        {isReady && <ChannelGrid />}

        {/* Master Section */}
        {isReady && <MasterSection />}
      </Stack>
    </Container>
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
