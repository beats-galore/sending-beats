import { Group, Modal, Stack, Text } from '@mantine/core';
import { invoke } from '@tauri-apps/api/core';
import { useCallback } from 'react';

import { useMixerStore } from '../../../stores/mixer-store';
import { color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';

/**
 * Blocking prompt shown once the virtual audio driver has been installed.
 *
 * Installing the driver restarts coreaudiod, which invalidates the running
 * process's Core Audio client, so the driver cannot be used until Sendin Beats
 * is launched again. There is no dismiss path because nothing the user does in
 * this session will finish the setup.
 */
export const SystemAudioRestartModal = () => {
  const restartRequired = useMixerStore((state) => state.systemAudioRestartRequired);

  const handleQuit = useCallback(() => {
    void invoke('quit_application');
  }, []);

  return (
    <Modal
      opened={restartRequired}
      onClose={handleQuit}
      title="Restart required"
      centered
      withCloseButton={false}
      closeOnClickOutside={false}
      closeOnEscape={false}
    >
      <Stack gap="lg">
        <Text size="sm">
          The Sendin Beats virtual audio driver has been installed. macOS restarted its audio
          service during setup, so Sendin Beats needs to be launched again before system audio can
          be routed through the mixer.
        </Text>
        <Text size="sm" c={color.textDim}>
          Until then you will hear each source twice.
        </Text>
        <Group justify="flex-end">
          <ActionButton tone="hot" padding="9px 14px" size="xs" onClick={handleQuit}>
            QUIT SENDIN BEATS
          </ActionButton>
        </Group>
      </Stack>
    </Modal>
  );
};
