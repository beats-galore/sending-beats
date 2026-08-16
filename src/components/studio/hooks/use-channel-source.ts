import { useCallback, useMemo } from 'react';

import { useApplicationAudio, useAudioDevices } from '../../../hooks';
import { audioService } from '../../../services';
import { useMixerStore } from '../../../stores/mixer-store';
import { asDeviceIdentifier } from '../../../types/device-identifier';
import { isVirtualDevice } from '../../../types/audio.types';
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

  /**
   * What this channel could be fed by, in groups.
   *
   * Grouped rather than one flat list for the same reason the dock has one
   * button per kind: a machine with a dozen loopback drivers turns a single
   * list into something you read rather than scan.
   */
  const options = useMemo(() => {
    const asOption = (device: { id: string; name: string }) => ({
      value: device.id,
      label: device.name,
    });

    // Keyed by bundle identifier, not PID: a PID is only valid for one launch,
    // so a source saved against it can never be restored on the next startup.
    const applications = applicationAudio.knownApps
      .filter((app) => app.bundle_id)
      .map((app) => ({ value: `app-${app.bundle_id}`, label: app.name }));

    const groups = [
      { group: 'Physical inputs', items: inputDevices.filter((d) => !isVirtualDevice(d)).map(asOption) },
      { group: 'Virtual inputs', items: inputDevices.filter(isVirtualDevice).map(asOption) },
      { group: 'Applications', items: applications },
      { group: 'File players', items: playerSources.options },
    ].filter((entry) => entry.items.length > 0);

    // A source that has gone away — unplugged device, or an application that is
    // not running — still needs an entry, or the select would silently show the
    // wrong source as patched.
    const identifier = configuredDevice?.deviceIdentifier;
    const present =
      identifier !== undefined &&
      groups.some((entry) => entry.items.some((item) => item.value === identifier));

    if (identifier !== undefined && !present) {
      groups.unshift({
        group: 'Unavailable',
        items: [
          {
            value: String(identifier),
            label: `${configuredDevice?.deviceName ?? String(identifier)} (unavailable)`,
          },
        ],
      });
    }

    return groups;
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
          isApplicationTap,
          channelId
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
