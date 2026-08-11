import { useCallback, useMemo } from 'react';

import { useApplicationAudio, useAudioDevices } from '../../../hooks';
import { audioService } from '../../../services';
import { useConfigurationStore } from '../../../stores/mixer-store';
import { asDeviceIdentifier } from '../../../types/device-identifier';

/**
 * The source patched into a channel, and the list of things it could be.
 *
 * Hardware inputs and application taps are both valid sources but are switched
 * through the same call — an identifier prefixed `app-` marks the tap case.
 */
export const useChannelSource = (channelId: number) => {
  const { inputDevices, refreshDevices } = useAudioDevices();
  const applicationAudio = useApplicationAudio();
  const { activeSession, updateConfiguredDevice, removeConfiguredDevice } = useConfigurationStore();

  const configuredDevice = useMemo(
    () =>
      activeSession?.configuredDevices.find(
        (device) => device.channelNumber === channelId && device.isInput
      ) ?? null,
    [activeSession, channelId]
  );

  const options = useMemo(() => {
    const hardware = inputDevices.map((device) => ({ value: device.id, label: device.name }));

    // A device that has gone away still needs an entry, or the select would
    // silently show the wrong source as patched.
    if (configuredDevice && !configuredDevice.deviceIdentifier.startsWith('app-')) {
      const present = inputDevices.some(
        (device) => device.id === configuredDevice.deviceIdentifier
      );
      if (!present) {
        hardware.unshift({
          value: configuredDevice.deviceIdentifier,
          label: `${configuredDevice.deviceName ?? configuredDevice.deviceIdentifier} (unavailable)`,
        });
      }
    }

    const applications = applicationAudio.knownApps.map((app) => ({
      value: `app-${app.pid}`,
      label: `App: ${app.name}`,
    }));

    return [...hardware, ...applications];
  }, [inputDevices, applicationAudio.knownApps, configuredDevice]);

  const setSource = useCallback(
    async (deviceId: string) => {
      const current = configuredDevice?.deviceIdentifier ?? null;
      if (!deviceId || deviceId === current) {
        return;
      }

      const isApplicationTap = deviceId.startsWith('app-');
      if (!isApplicationTap && !inputDevices.some((device) => device.id === deviceId)) {
        return;
      }

      try {
        if (current) {
          removeConfiguredDevice(current);
        }
        const updated = await audioService.switchInputStream(
          current,
          asDeviceIdentifier(deviceId),
          isApplicationTap
        );
        if (updated) {
          updateConfiguredDevice(updated);
        }
      } catch (error) {
        console.error(`Failed to switch input for channel ${channelId}:`, error);
      }
    },
    [channelId, configuredDevice, inputDevices, removeConfiguredDevice, updateConfiguredDevice]
  );

  const refresh = useCallback(() => {
    void refreshDevices();
    void applicationAudio.actions.refreshApplications();
  }, [refreshDevices, applicationAudio.actions]);

  return {
    configuredDevice,
    options,
    setSource,
    refresh,
    isApplicationTap: Boolean(configuredDevice?.deviceIdentifier.startsWith('app-')),
  };
};
