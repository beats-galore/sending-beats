// Mixer control buttons (Start/Stop/Add Channel)
import { Group, Button } from '@mantine/core';
import { IconPlayerPlay, IconPlayerStop, IconPlus } from '@tabler/icons-react';
import { memo } from 'react';
import { useMixerControls } from '../../hooks';

export const MixerControls = memo(() => {
  const { isReady, isRunning, onStart, onStop, onAddChannel } = useMixerControls();
    return (
      <Group>
        <Button
          leftSection={isRunning ? <IconPlayerStop size={16} /> : <IconPlayerPlay size={16} />}
          color={isRunning ? 'red' : 'green'}
          onClick={isRunning ? onStop : onStart}
          disabled={!isReady}
          variant={isRunning ? 'filled' : 'outline'}
        >
          {isRunning ? 'Stop Mixer' : 'Start Mixer'}
        </Button>

        <Button
          leftSection={<IconPlus size={16} />}
          onClick={onAddChannel}
          disabled={!isReady}
          variant="outline"
        >
          Add Channel
        </Button>
      </Group>
    );
  }
);
