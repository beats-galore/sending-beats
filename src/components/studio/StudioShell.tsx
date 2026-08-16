import { Box, Stack } from '@mantine/core';
import { useEffect } from 'react';

import { useAudioDevicesStatus, useMixerInitialization, useMixerRunningState } from '../../hooks';
import { useVUChannelStream } from '../../hooks/use-vu-channel-stream';
import { useStudioStore } from '../../stores/studio-store';
import { color } from '../../theme/tokens';

import { CastView } from './cast/CastView';
import { QueuesView } from './queues/QueuesView';
import { DevicesView } from './devices/DevicesView';
import { useStudioHotkeys } from './hooks/use-studio-hotkeys';
import { PatchView } from './patch/PatchView';
import { SetupView } from './setup/SetupView';
import { DeviceRestoreWarning } from './shell/DeviceRestoreWarning';
import { StudioError } from './shell/StudioError';
import { SystemAudioRestartModal } from './shell/SystemAudioRestartModal';
import { SystemAudioWarning } from './shell/SystemAudioWarning';
import { TopBar } from './shell/TopBar';
import { TapeView } from './tape/TapeView';

/** The application: chrome across the top, one of five views below it. */
export const StudioShell = () => {
  const view = useStudioStore((state) => state.view);

  const { error: devicesError } = useAudioDevicesStatus();
  const { isReady, error: mixerError, initialize } = useMixerInitialization();
  const isRunning = useMixerRunningState();

  useVUChannelStream(isRunning);
  useStudioHotkeys();

  useEffect(() => {
    if (!isReady && !mixerError) {
      void initialize();
    }
  }, [isReady, mixerError, initialize]);

  const renderView = () => {
    if (devicesError) {
      return <StudioError title="DEVICE ERROR" message={devicesError} />;
    }
    if (mixerError) {
      return (
        <StudioError title="ENGINE ERROR" message={mixerError} onRetry={() => void initialize()} />
      );
    }

    switch (view) {
      case 'tape':
        return <TapeView />;
      case 'cast':
        return <CastView />;
      case 'queues':
        return <QueuesView />;
      case 'devices':
        return <DevicesView />;
      case 'setup':
        return <SetupView />;
      case 'patch':
      default:
        return <PatchView ready={isReady} />;
    }
  };

  return (
    <Stack h="100vh" gap={0} style={{ background: color.bg, overflow: 'hidden' }}>
      <TopBar />
      <DeviceRestoreWarning />
      <SystemAudioWarning />
      <Box style={{ flex: 1, minHeight: 0, display: 'flex', alignItems: 'stretch' }}>
        {renderView()}
      </Box>
      <SystemAudioRestartModal />
    </Stack>
  );
};
