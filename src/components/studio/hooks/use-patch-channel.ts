import { useCallback, useEffect, useMemo } from 'react';

import { useChannelLevels } from '../../../hooks';
import {
  audioEffectsDefaultActions,
  useAudioEffectsDefaultStore,
} from '../../../stores/audio-effects-default-store';
import { useConfigurationStore } from '../../../stores/mixer-store';
import type { AudioChannel } from '../../../types';
import { dbToLinear, linearToDb } from '../format';

/**
 * Everything one patchbay channel node needs, assembled from the three places
 * the data actually lives.
 *
 * A channel's identity and processing come from the mixer config, but its gain,
 * pan, mute and solo belong to the effects row of the configured device patched
 * into it — so a channel with no device yet has controls that cannot be driven.
 */
export const usePatchChannel = (channel: AudioChannel) => {
  const { activeSession } = useConfigurationStore();
  const effectsById = useAudioEffectsDefaultStore((state) => state.effectsById);
  const levels = useChannelLevels(channel.id);

  const { loadEffects, updateGain, updatePan, setEffectsEnabled, toggleMute, toggleSolo } =
    audioEffectsDefaultActions;

  const device = useMemo(
    () =>
      activeSession?.configuredDevices.find(
        (configured) => configured.channelNumber === channel.id && configured.isInput
      ) ?? null,
    [activeSession, channel.id]
  );

  // Read straight out of the subscribed map rather than through the store getter,
  // so the lookup re-runs the moment a device's effects row arrives.
  const effects = useMemo(
    () =>
      device
        ? (Object.values(effectsById).find((effect) => effect.deviceId === device.id) ?? null)
        : null,
    [device, effectsById]
  );

  // A device registered after the initial load has no effects row yet, so this
  // reloads on device change as well as on configuration change.
  useEffect(() => {
    if (activeSession?.configuration.id) {
      void loadEffects(activeSession.configuration.id);
    }
  }, [activeSession?.configuration.id, device?.id, loadEffects]);

  const configurationId = activeSession?.configuration.id;
  const canEdit = Boolean(effects && device && configurationId);

  useEffect(() => {
    if (!device) {
      return;
    }
    void setEffectsEnabled(device.id, channel.effects_enabled);
  }, [device, channel.effects_enabled, setEffectsEnabled]);

  const setGain = useCallback(
    (gainDb: number) => {
      if (!effects || !device || !configurationId) {
        return;
      }
      void updateGain(effects.id, device.id, configurationId, dbToLinear(gainDb));
    },
    [effects, device, configurationId, updateGain]
  );

  const setPan = useCallback(
    (pan: number) => {
      if (!effects || !device || !configurationId) {
        return;
      }
      void updatePan(effects.id, device.id, configurationId, Math.round(pan * 100) / 100);
    },
    [effects, device, configurationId, updatePan]
  );

  const setMuted = useCallback(() => {
    if (!effects || !device || !configurationId) {
      return;
    }
    void toggleMute(effects.id, device.id, configurationId);
  }, [effects, device, configurationId, toggleMute]);

  const setSolo = useCallback(() => {
    if (!effects || !device || !configurationId) {
      return;
    }
    void toggleSolo(effects.id, device.id, configurationId);
  }, [effects, device, configurationId, toggleSolo]);

  return {
    device,
    levels,
    canEdit,
    gainDb: effects ? linearToDb(effects.gain) : 0,
    pan: effects?.pan ?? 0,
    muted: effects?.muted ?? false,
    solo: effects?.solo ?? false,
    isMono: device?.channelFormat === 'mono',
    sourceName: device?.deviceName ?? 'No input',
    setGain,
    setPan,
    setMuted,
    setSolo,
  };
};
