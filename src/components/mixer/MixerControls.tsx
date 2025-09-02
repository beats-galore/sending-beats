// Mixer control buttons (Add Channel) - No start/stop needed for always-running mixer
import { Group, Button } from '@mantine/core';
import { IconPlus, IconBug, IconBugOff } from '@tabler/icons-react';
import { invoke } from '@tauri-apps/api/core';
import { memo, useState, useEffect } from 'react';

import { useMixerControls } from '../../hooks';
import { DebugLogModal } from '../DebugLogModal';

export const MixerControls = memo(() => {
  const { isReady, onAddChannel } = useMixerControls();
  const [showDebugModal, setShowDebugModal] = useState(false);

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

      <Button
        leftSection={<IconBug size={16} />}
        onClick={() => setShowDebugModal(true)}
        variant="filled"
        color="yellow"
        size="sm"
      >
        Set debug config
      </Button>
      <DebugLogModal opened={showDebugModal} onClose={() => setShowDebugModal(false)} />
    </Group>
  );
});
