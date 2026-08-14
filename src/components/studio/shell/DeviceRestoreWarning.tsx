import { Alert, List, Text } from '@mantine/core';
import { IconPlugConnectedX } from '@tabler/icons-react';
import { useCallback } from 'react';

import { useMixerStore } from '../../../stores/mixer-store';

/**
 * Notice shown when devices saved in the session could not be reconnected.
 *
 * Without it a disconnected device is invisible: the session still lists the
 * device, so the mixer renders the source as patched while no audio reaches it.
 */
export const DeviceRestoreWarning = () => {
  const failures = useMixerStore((state) => state.deviceRestoreFailures);
  const clearDeviceRestoreFailures = useMixerStore((state) => state.clearDeviceRestoreFailures);

  const handleDismiss = useCallback(() => {
    clearDeviceRestoreFailures();
  }, [clearDeviceRestoreFailures]);

  if (failures.length === 0) {
    return null;
  }

  const hasOutputFailure = failures.some((failure) => !failure.isInput);

  return (
    <Alert
      icon={<IconPlugConnectedX size={16} />}
      title={
        failures.length === 1
          ? '1 saved device could not be connected'
          : `${failures.length} saved devices could not be connected`
      }
      color="amber"
      variant="outline"
      radius="xl"
      m="4xl"
      withCloseButton
      onClose={handleDismiss}
    >
      <List size="sm" spacing="xs">
        {failures.map((failure) => (
          <List.Item key={failure.deviceIdentifier}>
            <Text span fw={600}>
              {failure.deviceName ?? failure.deviceIdentifier}
            </Text>{' '}
            <Text span c="dimmed">
              ({failure.isInput ? 'input' : 'output'}) — {failure.reason}
            </Text>
          </List.Item>
        ))}
      </List>
      {hasOutputFailure ? (
        <Text size="sm" mt="sm">
          The mix has nowhere to go until you select an output device that is connected.
        </Text>
      ) : (
        <Text size="sm" mt="sm">
          Reconnect the device and reselect it on its channel to try again.
        </Text>
      )}
    </Alert>
  );
};
