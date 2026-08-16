import { useCallback, useMemo } from 'react';

import { useApplicationAudio, useAudioDevices } from '../../../hooks';
import { audioService } from '../../../services';
import { channelTargetKey } from '../../../services/patch-color-service';
import { useMixerStore } from '../../../stores/mixer-store';
import { usePatchColorStore } from '../../../stores/patch-color-store';
import type { AudioDeviceInfo } from '../../../types/audio.types';
import { isVirtualDevice, transportLabel } from '../../../types/audio.types';
import { asDeviceIdentifier } from '../../../types/device-identifier';
import { patchColorOf } from './use-patch-color';
import { NEW_PLAYER_VALUE, usePlayerSources } from './use-player-sources';

/** Where something already on the patch sits, for the badge beside its name. */
export type PatchedAt = {
  /** Position in the source column, which is the number shown. */
  position: number;
  color: string;
};

/** One thing that could be patched, or already is. */
export type SourceOption = {
  value: string;
  label: string;
  /** How it is attached, or what it is — the second line in the picker. */
  detail: string;
  /**
   * Where it already sits, when it is already on the patch.
   *
   * Present means the option is shown but cannot be chosen. Leaving these out
   * entirely is what makes a missing microphone read as a broken app rather
   * than as one that is already patched two strips up.
   */
  patchedAt?: PatchedAt;
};

/**
 * What can be added to the patch, split by what kind of thing it is.
 *
 * Four short lists rather than one long one. A studio has a handful of real
 * inputs and a dozen virtual ones it never uses on purpose, and asking for "a
 * microphone" should not mean reading past every loopback driver installed on
 * the machine to find it.
 */
export const useAddSource = () => {
  const { inputDevices } = useAudioDevices();
  const applicationAudio = useApplicationAudio();
  const activeSession = useMixerStore((state) => state.activeSession);
  const refreshChannels = useMixerStore((state) => state.refreshChannels);
  const updateConfiguredDevice = useMixerStore((state) => state.updateConfiguredDevice);
  const patchColors = usePatchColorStore((state) => state.colors);
  const playerSources = usePlayerSources(null);

  /** Where each patched identifier sits, by identifier. */
  const patched = useMemo(() => {
    const inputs = (activeSession?.configuredDevices ?? [])
      .filter((device) => device.isInput)
      .sort((left, right) => left.channelNumber - right.channelNumber);

    return new Map(
      inputs.map((device, position): [string, PatchedAt] => [
        String(device.deviceIdentifier),
        {
          position,
          color: patchColorOf(patchColors, channelTargetKey(device.channelNumber), position).value,
        },
      ])
    );
  }, [activeSession, patchColors]);

  const asOption = useCallback(
    (value: string, label: string, detail: string): SourceOption => {
      const at = patched.get(value);
      return at ? { value, label, detail, patchedAt: at } : { value, label, detail };
    },
    [patched]
  );

  const deviceOption = useCallback(
    (device: AudioDeviceInfo) => asOption(device.id, device.name, transportLabel(device.transport)),
    [asOption]
  );

  const physical = useMemo(
    () => inputDevices.filter((device) => !isVirtualDevice(device)).map(deviceOption),
    [inputDevices, deviceOption]
  );

  const virtual = useMemo(
    () => inputDevices.filter(isVirtualDevice).map(deviceOption),
    [inputDevices, deviceOption]
  );

  const applications = useMemo(
    () =>
      applicationAudio.knownApps
        .filter((app) => app.bundle_id)
        .map((app) => asOption(`app-${app.bundle_id}`, app.name, 'application')),
    [applicationAudio.knownApps, asOption]
  );

  // Players already on the patch are filtered out by `usePlayerSources`, which
  // also supplies the entry that makes a new one.
  const players = useMemo(
    () =>
      playerSources.options.map((option) =>
        option.value === NEW_PLAYER_VALUE
          ? { value: option.value, label: 'New queue', detail: 'creates one' }
          : asOption(option.value, option.label, 'queue')
      ),
    [playerSources.options, asOption]
  );

  /**
   * Patch a source, which is what brings its strip into being.
   *
   * No strip is made first: a channel exists because something is patched into
   * it, so attaching the device — which takes the next free channel number —
   * is the whole of adding a source.
   */
  const add = useCallback(
    async (value: string) => {
      const identifier = value === NEW_PLAYER_VALUE ? await playerSources.create() : value;
      if (!identifier) {
        return;
      }

      // Patching a channel to a queue is not enough on its own: the patch has
      // to record that it wants the queue, or nothing brings it back next time.
      if (value !== NEW_PLAYER_VALUE) {
        await playerSources.ensureOnPatch(identifier);
      }

      try {
        const attached = await audioService.switchInputStream(
          null,
          asDeviceIdentifier(identifier),
          identifier.startsWith('app-')
        );
        if (attached) {
          updateConfiguredDevice(attached);
        }
      } catch (error) {
        console.error('Failed to add the source:', error);
      }

      await refreshChannels();
    },
    [playerSources, refreshChannels, updateConfiguredDevice]
  );

  return { physical, virtual, applications, players, add };
};
