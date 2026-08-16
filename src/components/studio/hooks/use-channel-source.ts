import { useCallback, useMemo } from 'react';

import { useApplicationAudio, useAudioDevices } from '../../../hooks';
import { audioService } from '../../../services';
import { useMixerStore } from '../../../stores/mixer-store';
import { asDeviceIdentifier } from '../../../types/device-identifier';
import { patchedPlayerId } from '../../../types/file-player.types';
import { NEW_PLAYER_VALUE, usePlayerSources } from './use-player-sources';

/**
 * The source patched into a channel, and the list of things it could be.
 *
 * Hardware inputs, application taps and file players are all valid sources and
 * are switched through the same call. Which kind an identifier names is read
 * from the identifier itself: `app-` marks a tap, and a player is recognised by
 * being in the list of players.
 */
export const useChannelSource = (channelId: number) => {
  const { inputDevices, refreshDevices, disconnectedDeviceIds } = useAudioDevices();
  const applicationAudio = useApplicationAudio();
  const activeSession = useMixerStore((state) => state.activeSession);
  const updateConfiguredDevice = useMixerStore((state) => state.updateConfiguredDevice);
  const removeConfiguredDevice = useMixerStore((state) => state.removeConfiguredDevice);
  const restoreFailures = useMixerStore((state) => state.deviceRestoreFailures);

  const configuredDevice = useMemo(
    () =>
      activeSession?.configuredDevices.find(
        (device) => device.channelNumber === channelId && device.isInput
      ) ?? null,
    [activeSession, channelId]
  );

  const playerSources = usePlayerSources(configuredDevice?.deviceIdentifier ?? null);
  const playerId = patchedPlayerId(configuredDevice?.deviceIdentifier, playerSources.players);

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

    const available = [...hardware, ...applications, ...playerSources.options];

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
  }, [inputDevices, applicationAudio.knownApps, configuredDevice, playerSources.options]);

  // The channel still holds this source, but nothing is feeding it: either the
  // device vanished from enumeration, or restoring it on startup failed.
  const unavailable = useMemo(() => {
    if (!configuredDevice) {
      return null;
    }

    const identifier = configuredDevice.deviceIdentifier;

    // The watcher clears this the moment a device reappears and only puts it
    // back when rebuilding the stream failed, so it outranks presence: the
    // hardware is there but nothing is reading it.
    if (disconnectedDeviceIds.includes(identifier)) {
      return 'Device reconnected but its stream could not be restored';
    }

    const present = (() => {
      // A player is software this app is running. It cannot be unplugged, and
      // there is nothing to enumerate it against.
      if (playerId) {
        return true;
      }

      if (identifier.startsWith('app-')) {
        return applicationAudio.knownApps.some((app) => `app-${app.bundle_id}` === identifier);
      }

      return inputDevices.some((device) => device.id === identifier);
    })();

    // Presence wins over the restore record, which is only a snapshot of what
    // failed at startup — a device reconnected since then is available again.
    if (present) {
      return null;
    }

    const restoreFailure = restoreFailures.find(
      (failure) => failure.deviceIdentifier === identifier
    );

    return restoreFailure?.reason ?? 'Device is not currently available';
  }, [
    configuredDevice,
    restoreFailures,
    disconnectedDeviceIds,
    inputDevices,
    applicationAudio.knownApps,
    playerId,
  ]);

  const setSource = useCallback(
    async (deviceId: string) => {
      const current = configuredDevice?.deviceIdentifier ?? null;
      if (!deviceId || deviceId === current) {
        return;
      }

      // Making a player is choosing one: the entry stands for the player it is
      // about to create, so the channel is patched to what comes back.
      const identifier =
        deviceId === NEW_PLAYER_VALUE ? await playerSources.create() : deviceId;
      if (!identifier) {
        return;
      }

      const isApplicationTap = identifier.startsWith('app-');
      const isPlayer =
        identifier !== deviceId || playerSources.players.some((player) => player.id === identifier);

      if (!isApplicationTap && !isPlayer && !inputDevices.some((device) => device.id === identifier)) {
        return;
      }

      try {
        if (current) {
          removeConfiguredDevice(current);
        }
        const updated = await audioService.switchInputStream(
          current,
          asDeviceIdentifier(identifier),
          isApplicationTap
        );
        if (updated) {
          updateConfiguredDevice(updated);
        }
      } catch (error) {
        console.error(`Failed to switch input for channel ${channelId}:`, error);
      }
    },
    [
      channelId,
      configuredDevice,
      inputDevices,
      playerSources,
      removeConfiguredDevice,
      updateConfiguredDevice,
    ]
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
    /** The player feeding this channel, or null when something else is. */
    playerId,
    /** Why the patched source is not carrying audio, or null when it is fine. */
    unavailableReason: unavailable,
  };
};
