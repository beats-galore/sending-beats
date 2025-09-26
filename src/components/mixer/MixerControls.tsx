// Mixer control buttons (Add Channel) - No start/stop needed for always-running mixer
import { Group, Button, Stack, Grid } from '@mantine/core';
import { IconPlus, IconBug } from '@tabler/icons-react';
import { memo, useState } from 'react';

import { useMixerControls } from '../../hooks';
import { ConfigurationSaver } from '../ConfigurationSaver';
import { ConfigurationSelector } from '../ConfigurationSelector';
import { DebugLogModal } from '../DebugLogModal';
import { SaveAsNewConfiguration } from '../SaveAsNewConfiguration';

export const MixerControls = memo(() => {
  const { isReady, onAddChannel } = useMixerControls();
  const [showDebugModal, setShowDebugModal] = useState(false);

  const handleConfigurationSelect = (configId: string) => {
    console.log('Configuration selected:', configId);
    // TODO: Refresh mixer UI or trigger reload
  };

  const handleConfigurationSaved = () => {
    console.log('Configuration saved');
    // TODO: Show success notification
  };

  const handleConfigurationCreated = () => {
    console.log('New configuration created');
    // TODO: Show success notification and refresh list
  };

  return (
    <Stack gap="md">
      {/* Configuration Controls */}
      <Grid>
        <Grid.Col span={{ base: 12, md: 4 }}>
          <ConfigurationSelector
            onConfigurationSelect={handleConfigurationSelect}
          />
        </Grid.Col>
        <Grid.Col span={{ base: 12, md: 4 }}>
          <ConfigurationSaver
            onConfigurationSaved={handleConfigurationSaved}
          />
        </Grid.Col>
        <Grid.Col span={{ base: 12, md: 4 }}>
          <SaveAsNewConfiguration
            onConfigurationCreated={handleConfigurationCreated}
          />
        </Grid.Col>
      </Grid>

      {/* Mixer Action Controls */}
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
    </Stack>
  );
});
