// Mixer control buttons (Start/Stop/Add Channel)
import { memo } from 'react';
import { Group, Button } from '@mantine/core';
import { IconPlayerPlay, IconPlayerStop, IconPlus } from '@tabler/icons-react';

type MixerControlsProps = {
  isReady: boolean;
  isRunning: boolean;
  onStart: () => void;
  onStop: () => void;
  onAddChannel: () => void;
};

export const MixerControls = memo<MixerControlsProps>(({
  isReady,
  isRunning,
  onStart,
  onStop,
  onAddChannel
}) => {
  return (
    <Group>
      <Button
        leftSection={isRunning ? <IconPlayerStop size={16} /> : <IconPlayerPlay size={16} />}
        color={isRunning ? "red" : "green"}
        onClick={isRunning ? onStop : onStart}
        disabled={!isReady}
        variant={isRunning ? "filled" : "outline"}
      >
        {isRunning ? "Stop Mixer" : "Start Mixer"}
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
});