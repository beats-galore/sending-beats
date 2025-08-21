// Mixer control buttons (Add Channel) - No start/stop needed for always-running mixer
import { Group, Button } from '@mantine/core';
import { IconPlus } from '@tabler/icons-react';
import { memo } from 'react';
import { useMixerControls } from '../../hooks';

export const MixerControls = memo(() => {
  const { isReady, onAddChannel } = useMixerControls();
  return (
    <Group>
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
