// Mixer control buttons (Add Channel) - No start/stop needed for always-running mixer
import { Group, Button } from '@mantine/core';
import { IconPlus, IconBug, IconBugOff } from '@tabler/icons-react';
import { memo, useState, useEffect } from 'react';
import { useMixerControls } from '../../hooks';
import { invoke } from '@tauri-apps/api/core';

export const MixerControls = memo(() => {
  const { isReady, onAddChannel } = useMixerControls();
  const [debugEnabled, setDebugEnabled] = useState(false);

  useEffect(() => {
    // Load initial debug state
    invoke('get_audio_debug_enabled').then((enabled: boolean) => {
      setDebugEnabled(enabled);
    });
  }, []);

  const toggleDebug = async () => {
    const newState = !debugEnabled;
    await invoke('set_audio_debug_enabled', { enabled: newState });
    setDebugEnabled(newState);
  };

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
        leftSection={debugEnabled ? <IconBug size={16} /> : <IconBugOff size={16} />}
        onClick={toggleDebug}
        variant={debugEnabled ? "filled" : "outline"}
        color={debugEnabled ? "yellow" : "gray"}
        size="sm"
      >
        Debug {debugEnabled ? "ON" : "OFF"}
      </Button>
    </Group>
  );
});
