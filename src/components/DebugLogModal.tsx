import { Button, Group, Modal, Stack, Switch } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { invoke } from '@tauri-apps/api/core';
import { useState, useEffect, useCallback } from 'react';

type DebugLogConfig = {
  audio: boolean;
  device: boolean;
};
const useStyles = createStyles(() => ({
  modalContent: {
    maxHeight: '80vh',
  },
}));

export const DebugLogModal = ({ opened, onClose }: { opened: boolean; onClose: () => void }) => {
  const { classes } = useStyles();
  const [debugLogConfig, setDebugLogConfig] = useState<DebugLogConfig>({
    audio: false,
    device: false,
  });

  useEffect(() => {
    // Load initial debug state
    console.log('Loading debug log config');
    void invoke<DebugLogConfig>('get_debug_log_config')
      .then((config) => {
        console.log('Debug log config loaded:', config);
        setDebugLogConfig(config);
      })
      .catch((error) => {
        console.error('Error loading debug log config:', error);
      });
  }, []);

  const saveDebugLogConfig = useCallback(() => {
    const runToggle = async () => {
      await invoke('set_debug_log_config', { ...debugLogConfig });
      onClose();
    };

    void runToggle();
  }, [debugLogConfig, onClose]);

  return (
    <Modal opened={opened} onClose={onClose} title="Debug Log Config">
      <Stack gap="md" className={classes.modalContent}>
        <Switch
          label="Audio"
          checked={debugLogConfig.audio}
          onChange={() => setDebugLogConfig((s) => ({ ...s, audio: !s.audio }))}
        />
        <Switch
          label="Device"
          checked={debugLogConfig.device}
          onChange={() => setDebugLogConfig((s) => ({ ...s, device: !s.device }))}
        />
      </Stack>
      <Group justify="flex-end" gap="sm" pt="md">
        <Button variant="subtle" onClick={onClose}>
          Cancel
        </Button>
        <Button onClick={saveDebugLogConfig}>Save</Button>
      </Group>
    </Modal>
  );
};
