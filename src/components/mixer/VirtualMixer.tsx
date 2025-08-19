// Professional Virtual Mixer - Refactored with modern architecture
import { memo, useEffect } from 'react';
import { Container, Title, Stack, Alert, Button, Group } from '@mantine/core';
import { IconAlertCircle, IconRefresh } from '@tabler/icons-react';
import { ErrorBoundary, FullScreenLoader } from '../layout';
import { useAudioDevices, useMixerState, useVUMeterData } from '../../hooks';
import { ChannelGrid } from './ChannelGrid';
import { MasterSection } from './MasterSection';
import { MixerControls } from './MixerControls';

const VirtualMixer = memo(() => {
  const { 
    inputDevices, 
    outputDevices, 
    isLoading: devicesLoading, 
    error: devicesError,
    refreshDevices 
  } = useAudioDevices();

  
  const { 
    config, 
    isRunning,
    isReady, 
    error: mixerError,
    initialize,
    start,
    stop,
    createChannel
  } = useMixerState();
  
  // Start VU meter polling when running
  useVUMeterData(isRunning);

  // Initialize mixer on mount
  useEffect(() => {
    if (!config && !devicesLoading && !mixerError) {
      initialize();
    }
  }, [config, devicesLoading, mixerError, initialize]);

  // Handle loading states
  if (devicesLoading) {
    return <FullScreenLoader message="Loading audio devices..." />;
  }

  if (devicesError) {
    return (
      <Container fluid p="md" style={{ minHeight: "100vh", display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Alert 
          icon={<IconAlertCircle size={16} />} 
          title="Device Error" 
          color="red"
          style={{ maxWidth: 400 }}
        >
          {devicesError}
          <Group mt="md">
            <Button 
              color="red" 
              leftSection={<IconRefresh size={16} />}
              onClick={refreshDevices}
            >
              Retry
            </Button>
          </Group>
        </Alert>
      </Container>
    );
  }

  if (mixerError) {
    return (
      <Container fluid p="md" style={{ minHeight: "100vh", display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Alert 
          icon={<IconAlertCircle size={16} />} 
          title="Mixer Error" 
          color="red"
          style={{ maxWidth: 400 }}
        >
          {mixerError}
          <Group mt="md">
            <Button 
              color="red" 
              leftSection={<IconRefresh size={16} />}
              onClick={initialize}
            >
              Retry
            </Button>
          </Group>
        </Alert>
      </Container>
    );
  }

  return (
    <Container fluid p="md" style={{ minHeight: "100vh", maxWidth: "100%" }}>
      <Stack gap="lg" w="100%">
        <Title order={1} c="blue">Virtual Mixer</Title>
        
        {/* Mixer Controls */}
        <MixerControls
          isReady={isReady}
          isRunning={isRunning}
          onStart={start}
          onStop={stop}
          onAddChannel={createChannel}
        />
        
        {/* Channel Grid */}
        {config && (
          <ChannelGrid
            channels={config.channels}
            inputDevices={inputDevices}
            onRefreshDevices={refreshDevices}
          />
        )}
        
        {/* Master Section */}
        {config && (
          <MasterSection
            mixerConfig={config}
            outputDevices={outputDevices}
            onRefreshDevices={refreshDevices}
          />
        )}
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