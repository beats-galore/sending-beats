import { useCallback, useMemo } from 'react';

import { useApplicationAudio, useAudioDevices } from '../../../hooks';
import { audioService } from '../../../services';
import { useConfigurationStore, useMixerStore } from '../../../stores/mixer-store';
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
  const restoreFailures = useMixerStore((state) => state.deviceRestoreFailures);

  const configuredDevice = useMemo(
    () =>
      activeSession?.configuredDevices.find(
        (device) => device.channelNumber === channelId && device.isInput
      ) ?? null,
    [activeSession, channelId]
  );

  const options = useMemo(() => {
    const hardware = inputDevices.map((device) => ({ value: device.id, label: device.name }));

    // Keyed by bundle identifier, not PID: a PID is only valid for one launch,
    // so a source saved against it can never be restored on the next startup.
    const applications = applicationAudio.knownApps
      .filter((app) => app.bundle_id)
      .map((app) => ({
        value: `app-${app.bundle_id}`,
        label: `App: ${app.name}`,
      }));

    const available = [...hardware, ...applications];

    // A source that has gone away — unplugged device, or an application that is
    // not running — still needs an entry, or the select would silently show the
    // wrong source as patched.
    if (configuredDevice) {
      const present = available.some(
        (option) => option.value === configuredDevice.deviceIdentifier
      );
      if (!present) {
        available.unshift({
          value: configuredDevice.deviceIdentifier,
          label: `${configuredDevice.deviceName ?? configuredDevice.deviceIdentifier} (unavailable)`,
        });
      }
    }

    return available;
  }, [inputDevices, applicationAudio.knownApps, configuredDevice]);

  // The channel still holds this source, but nothing is feeding it: either the
  // device vanished from enumeration, or restoring it on startup failed.
  const unavailable = useMemo(() => {
    if (!configuredDevice) {
      return null;
    }

    const identifier = configuredDevice.deviceIdentifier;
    const restoreFailure = restoreFailures.find(
      (failure) => failure.deviceIdentifier === identifier
    );
    if (restoreFailure) {
      return restoreFailure.reason;
    }

    const isApp = identifier.startsWith('app-');
    const present = isApp
      ? applicationAudio.knownApps.some((app) => `app-${app.bundle_id}` === identifier)
      : inputDevices.some((device) => device.id === identifier);

    return present ? null : 'Device is not currently available';
  }, [configuredDevice, restoreFailures, inputDevices, applicationAudio.knownApps]);

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
    /** Why the patched source is not carrying audio, or null when it is fine. */
    unavailableReason: unavailable,
  };
};
