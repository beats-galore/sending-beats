import { Group, Select, ActionIcon, Box } from '@mantine/core';
import { IconRefresh } from '@tabler/icons-react';
import { memo, useCallback, useMemo } from 'react';

import { useAudioDevices, useApplicationAudio } from '../../hooks';
import { audioService } from '../../services';
import { useConfigurationStore } from '../../stores/mixer-store';

import type { ConfiguredAudioDevice } from '../../types/db';
import type { Identifier } from '../../types/util.types';

type InputAudioDeviceSelectProps = {
  channelId: number;
  className?: string;
};

export const InputAudioDeviceSelect = memo<InputAudioDeviceSelectProps>(
  ({ channelId, className }) => {
    const { inputDevices, refreshDevices } = useAudioDevices();
    const { activeSession, updateConfiguredDevice, removeConfiguredDevice } =
      useConfigurationStore();
    const applicationAudio = useApplicationAudio();

    const configuredInputDevice = useMemo(() => {
      if (!activeSession?.configuredDevices) {
        console.log(`❌ No activeSession or configuredDevices for channel ${channelId}`);
        return null;
      }
      const device = activeSession.configuredDevices.find(
        (d) => d.channelNumber === channelId && d.isInput
      );
      console.log(`🔍 configuredInputDevice for channel ${channelId}:`, device);
      return device;
    }, [activeSession, channelId]);

    const handleInputDeviceChange = useCallback(
      async (deviceId: Identifier<ConfiguredAudioDevice> | null) => {
        console.log(
          `🔧 FRONTEND: handleInputDeviceChange called for channel ${channelId} with deviceId:`,
          deviceId
        );
        if (deviceId) {
          const isDeviceAvailable = inputDevices.some((device) => device.id === deviceId);
          if (!isDeviceAvailable && !deviceId.startsWith('app-')) {
            console.warn(`⚠️ Attempted to select unavailable device: ${deviceId}`);
            return;
          }

          try {
            const currentDeviceId = configuredInputDevice?.deviceIdentifier ?? null;
            const isAppAudio = deviceId.startsWith('app-');
            console.log(
              `🔧 FRONTEND CH${channelId}: Switching input device: ${currentDeviceId} → ${deviceId}${isAppAudio ? ' (app audio)' : ''}`
            );
            console.log(
              `🔧 FRONTEND CH${channelId}: configuredInputDevice:`,
              configuredInputDevice
            );
            console.log(
              `🔧 FRONTEND CH${channelId}: activeSession devices:`,
              activeSession?.configuredDevices
            );

            if (currentDeviceId) {
              removeConfiguredDevice(currentDeviceId);
              console.log(`🗑️ Removed old device from UI state: ${currentDeviceId}`);
            }

            const updatedDevice = await audioService.switchInputStream(
              currentDeviceId,
              deviceId,
              isAppAudio
            );
            console.debug(`✅ Channel ${channelId} input device switched to: ${deviceId}`);

            if (updatedDevice) {
              updateConfiguredDevice(updatedDevice);
              console.log(`🔄 Updated UI state with new device configuration:`, updatedDevice);
            }
          } catch (error) {
            console.error(`❌ Failed to switch input device for channel ${channelId}:`, error);
          }
        } else {
          console.log(`🔧 FRONTEND: deviceId is null, not setting input device`);
        }
      },
      [
        channelId,
        configuredInputDevice,
        inputDevices,
        updateConfiguredDevice,
        removeConfiguredDevice,
        activeSession?.configuredDevices,
      ]
    );

    const inputDeviceOptions = useMemo(() => {
      const hardwareOptions = inputDevices.map((device) => ({
        value: device.id,
        label: device.name.length > 20 ? `${device.name.substring(0, 20)}...` : device.name,
      }));

      if (configuredInputDevice && !configuredInputDevice.deviceIdentifier.startsWith('app-')) {
        const isDeviceAvailable = inputDevices.some(
          (device) => device.id === configuredInputDevice.deviceIdentifier
        );
        if (!isDeviceAvailable) {
          const deviceName =
            configuredInputDevice.deviceName ?? configuredInputDevice.deviceIdentifier;
          hardwareOptions.unshift({
            value: configuredInputDevice.deviceIdentifier,
            label: `${deviceName} (unavailable)`,
          });
        }
      }

      const appOptions = applicationAudio.knownApps.map((app) => ({
        value: `app-${app.pid}`,
        label: app.name.length > 20 ? `${app.name.substring(0, 20)}...` : app.name,
      }));

      console.log('🎛️ InputAudioDeviceSelect device options:', {
        hardware: hardwareOptions.length,
        applications: appOptions.length,
        totalKnownApps: applicationAudio.knownApps.length,
        configuredInputDevice,
      });

      if (appOptions.length > 0) {
        return [
          {
            group: 'Hardware Devices',
            items: hardwareOptions,
          },
          {
            group: 'Applications',
            items: appOptions,
          },
        ];
      }

      return hardwareOptions;
    }, [inputDevices, applicationAudio.knownApps, configuredInputDevice]);

    return (
      <Box>
        <Group gap={4} align="center">
          <Select
            size="xs"
            placeholder="No Input"
            value={configuredInputDevice?.deviceIdentifier ?? null}
            onChange={(e) =>
              void handleInputDeviceChange(e as Identifier<ConfiguredAudioDevice> | null)
            }
            data={inputDeviceOptions}
            className={className}
          />
          <ActionIcon
            size="xs"
            onClick={() => {
              void refreshDevices();
              void applicationAudio.actions.refreshApplications();
            }}
            variant="subtle"
          >
            <IconRefresh size={10} />
          </ActionIcon>
        </Group>
      </Box>
    );
  }
);

InputAudioDeviceSelect.displayName = 'InputAudioDeviceSelect';
