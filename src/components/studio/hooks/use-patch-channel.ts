import { useCallback, useEffect, useMemo } from 'react';

import { useChannelLevels } from '../../../hooks';
import {
  audioEffectsDefaultActions,
  useAudioEffectsDefaultStore,
} from '../../../stores/audio-effects-default-store';
import { useMixerStore } from '../../../stores/mixer-store';
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
  const activeSession = useMixerStore((state) => state.activeSession);
  const effectsById = useAudioEffectsDefaultStore((state) => state.effectsById);
  const levels = useChannelLevels(channel.id);

  const {
    loadEffects,
    updateGain,
    updatePan,
    updateEffectsEnabled,
    toggleMute,
    toggleSolo,
    updateEq,
    updateCompressor,
    updateLimiter,
  } = audioEffectsDefaultActions;

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

  // No engine sync here: the switch lives on the effects row now, and the
  // worker reads it — with everything else on the strip — when the device is
  // attached. The interface only has to change it, not repeat it.
  const setEffectsEnabled = useCallback(
    (enabled: boolean) => {
      if (!effects || !device || !configurationId) {
        return;
      }
      void updateEffectsEnabled(effects.id, device.id, configurationId, enabled);
    },
    [effects, device, configurationId, updateEffectsEnabled]
  );

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

  const setEq = useCallback(
    (bands: { lowGain?: number; midGain?: number; highGain?: number }) => {
      if (!effects || !device || !configurationId) {
        return;
      }
      void updateEq(effects.id, device.id, configurationId, bands);
    },
    [effects, device, configurationId, updateEq]
  );

  const setCompressor = useCallback(
    (settings: {
      threshold?: number;
      ratio?: number;
      attack?: number;
      release?: number;
      enabled?: boolean;
    }) => {
      if (!effects || !device || !configurationId) {
        return;
      }
      void updateCompressor(effects.id, device.id, configurationId, settings);
    },
    [effects, device, configurationId, updateCompressor]
  );

  const setLimiter = useCallback(
    (settings: { threshold?: number; enabled?: boolean }) => {
      if (!effects || !device || !configurationId) {
        return;
      }
      void updateLimiter(effects.id, device.id, configurationId, settings);
    },
    [effects, device, configurationId, updateLimiter]
  );

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
    // The custom chain, read from the same effects row that carries the fader.
    // Defaults mirror the DSP constructors so an unpatched channel still draws.
    chain: {
      effectsEnabled: effects?.effectsEnabled ?? false,
      setEffectsEnabled,
      eqLowGain: effects?.eqLowGain ?? 0,
      eqMidGain: effects?.eqMidGain ?? 0,
      eqHighGain: effects?.eqHighGain ?? 0,
      compThreshold: effects?.compThreshold ?? -12,
      compRatio: effects?.compRatio ?? 4,
      compAttack: effects?.compAttack ?? 10,
      compRelease: effects?.compRelease ?? 200,
      compEnabled: effects?.compEnabled ?? false,
      limiterThreshold: effects?.limiterThreshold ?? -0.1,
      limiterEnabled: effects?.limiterEnabled ?? false,
      setEq,
      setCompressor,
      setLimiter,
    },
  };
};

/** The chain slice of a patch channel, as the inspector components take it. */
export type PatchChannelChain = ReturnType<typeof usePatchChannel>['chain'];
