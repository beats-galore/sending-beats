import { invoke } from '@tauri-apps/api/core';
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

import type {
  AudioEffectsDefault,
  CompressorUpdate,
  EqBandUpdate,
  LimiterUpdate,
} from '../types/db/audio-effects.types';
import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';
import type { ConfiguredAudioDevice } from '../types/db/configured-audio-devices.types';
import type { Uuid } from '../types/util.types';

type AudioEffectsDefaultStore = {
  effectsById: Record<string, AudioEffectsDefault>;
  isLoading: boolean;
  error: string | null;

  loadEffects: (configurationId: Uuid<AudioMixerConfiguration>) => Promise<void>;
  updateGain: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    gain: number
  ) => Promise<void>;
  updatePan: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    pan: number
  ) => Promise<void>;
  /** Switch a channel's effects chain on or off, in the engine and on its row. */
  updateEffectsEnabled: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    enabled: boolean
  ) => Promise<void>;
  toggleMute: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>
  ) => Promise<void>;
  toggleSolo: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>
  ) => Promise<void>;
  updateEq: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    bands: EqBandUpdate
  ) => Promise<void>;
  updateCompressor: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    settings: CompressorUpdate
  ) => Promise<void>;
  updateLimiter: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    settings: LimiterUpdate
  ) => Promise<void>;

  getEffectsByDeviceId: (deviceId: Uuid<ConfiguredAudioDevice>) => AudioEffectsDefault | null;
  setError: (error: string | null) => void;
  clearError: () => void;
};

const store = create<AudioEffectsDefaultStore>()(
  subscribeWithSelector((set, get) => ({
    effectsById: {},
    isLoading: false,
    error: null,

    loadEffects: async (configurationId: Uuid<AudioMixerConfiguration>) => {
      const { isLoading } = get();

      // Only guard against concurrent fetches. Effects rows are created when a
      // device is registered, so a load that ran before a device existed has no
      // entry for it — caching on configuration ID alone would strand that device
      // without effects for the rest of the session, leaving its gain, mute and
      // solo controls silently inert.
      if (isLoading) {
        return;
      }

      set({ isLoading: true, error: null });

      try {
        const effects = await invoke<AudioEffectsDefault[]>('get_audio_effects_defaults', {
          configurationId,
        });

        const effectsById = effects.reduce(
          (acc, effect) => {
            acc[effect.id] = effect;
            return acc;
          },
          {} as Record<string, AudioEffectsDefault>
        );

        set({ effectsById, isLoading: false });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to load effects';
        set({ error: errorMessage, isLoading: false });
        throw error;
      }
    },

    updateGain: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>,
      gain: number
    ) => {
      try {
        await invoke('update_audio_effects_default_gain', {
          effectsId,
          deviceId,
          configurationId,
          gain,
        });

        set((state) => ({
          effectsById: {
            ...state.effectsById,
            [effectsId]: {
              ...state.effectsById[effectsId],
              gain,
            },
          },
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to update gain';
        set({ error: errorMessage });
        throw error;
      }
    },

    updatePan: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>,
      pan: number
    ) => {
      try {
        await invoke('update_audio_effects_default_pan', {
          effectsId,
          deviceId,
          configurationId,
          pan,
        });

        set((state) => ({
          effectsById: {
            ...state.effectsById,
            [effectsId]: {
              ...state.effectsById[effectsId],
              pan,
            },
          },
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to update pan';
        set({ error: errorMessage });
        throw error;
      }
    },

    updateEffectsEnabled: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>,
      enabled: boolean
    ) => {
      try {
        await invoke('update_audio_effects_default_effects_enabled', {
          effectsId,
          deviceId,
          configurationId,
          enabled,
        });

        set((state) => ({
          effectsById: {
            ...state.effectsById,
            [effectsId]: {
              ...state.effectsById[effectsId],
              effectsEnabled: enabled,
            },
          },
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to switch effects';
        set({ error: errorMessage });
        throw error;
      }
    },

    toggleMute: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>
    ) => {
      const currentEffect = get().effectsById[effectsId];
      if (!currentEffect) {
        throw new Error(`Effect ${effectsId} not found`);
      }

      const newMuted = !currentEffect.muted;

      try {
        await invoke('update_audio_effects_default_mute', {
          effectsId,
          deviceId,
          configurationId,
          muted: newMuted,
        });

        set((state) => ({
          effectsById: {
            ...state.effectsById,
            [effectsId]: {
              ...state.effectsById[effectsId],
              muted: newMuted,
            },
          },
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to toggle mute';
        set({ error: errorMessage });
        throw error;
      }
    },

    toggleSolo: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>
    ) => {
      const currentEffect = get().effectsById[effectsId];
      if (!currentEffect) {
        throw new Error(`Effect ${effectsId} not found`);
      }

      const newSolo = !currentEffect.solo;

      try {
        await invoke('update_audio_effects_default_solo', {
          effectsId,
          deviceId,
          configurationId,
          solo: newSolo,
        });

        set((state) => ({
          effectsById: {
            ...state.effectsById,
            [effectsId]: {
              ...state.effectsById[effectsId],
              solo: newSolo,
            },
          },
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to toggle solo';
        set({ error: errorMessage });
        throw error;
      }
    },

    updateEq: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>,
      bands: EqBandUpdate
    ) => {
      try {
        await invoke('update_audio_effects_default_eq', {
          effectsId,
          deviceId,
          configurationId,
          lowGain: bands.lowGain,
          midGain: bands.midGain,
          highGain: bands.highGain,
        });

        set((state) => {
          const current = state.effectsById[effectsId];
          return {
            effectsById: {
              ...state.effectsById,
              [effectsId]: {
                ...current,
                eqLowGain: bands.lowGain ?? current.eqLowGain,
                eqMidGain: bands.midGain ?? current.eqMidGain,
                eqHighGain: bands.highGain ?? current.eqHighGain,
              },
            },
          };
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to update EQ';
        set({ error: errorMessage });
        throw error;
      }
    },

    updateCompressor: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>,
      settings: CompressorUpdate
    ) => {
      try {
        await invoke('update_audio_effects_default_compressor', {
          effectsId,
          deviceId,
          configurationId,
          threshold: settings.threshold,
          ratio: settings.ratio,
          attack: settings.attack,
          release: settings.release,
          enabled: settings.enabled,
        });

        set((state) => {
          const current = state.effectsById[effectsId];
          return {
            effectsById: {
              ...state.effectsById,
              [effectsId]: {
                ...current,
                compThreshold: settings.threshold ?? current.compThreshold,
                compRatio: settings.ratio ?? current.compRatio,
                compAttack: settings.attack ?? current.compAttack,
                compRelease: settings.release ?? current.compRelease,
                compEnabled: settings.enabled ?? current.compEnabled,
              },
            },
          };
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to update compressor';
        set({ error: errorMessage });
        throw error;
      }
    },

    updateLimiter: async (
      effectsId: Uuid<AudioEffectsDefault>,
      deviceId: Uuid<ConfiguredAudioDevice>,
      configurationId: Uuid<AudioMixerConfiguration>,
      settings: LimiterUpdate
    ) => {
      try {
        await invoke('update_audio_effects_default_limiter', {
          effectsId,
          deviceId,
          configurationId,
          threshold: settings.threshold,
          enabled: settings.enabled,
        });

        set((state) => {
          const current = state.effectsById[effectsId];
          return {
            effectsById: {
              ...state.effectsById,
              [effectsId]: {
                ...current,
                limiterThreshold: settings.threshold ?? current.limiterThreshold,
                limiterEnabled: settings.enabled ?? current.limiterEnabled,
              },
            },
          };
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to update limiter';
        set({ error: errorMessage });
        throw error;
      }
    },

    getEffectsByDeviceId: (deviceId: Uuid<ConfiguredAudioDevice>) => {
      const effects = Object.values(get().effectsById);
      return effects.find((effect) => effect.deviceId === deviceId) || null;
    },

    setError: (error: string | null) => set({ error }),
    clearError: () => set({ error: null }),
  }))
);

// Export the hook for state selection
export const useAudioEffectsDefaultStore = store;

// Export actions directly so they don't create dependencies
export const audioEffectsDefaultActions = {
  loadEffects: (configurationId: Uuid<AudioMixerConfiguration>) =>
    store.getState().loadEffects(configurationId),
  updateGain: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    gain: number
  ) => store.getState().updateGain(effectsId, deviceId, configurationId, gain),
  updatePan: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    pan: number
  ) => store.getState().updatePan(effectsId, deviceId, configurationId, pan),
  updateEffectsEnabled: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    enabled: boolean
  ) => store.getState().updateEffectsEnabled(effectsId, deviceId, configurationId, enabled),
  toggleMute: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>
  ) => store.getState().toggleMute(effectsId, deviceId, configurationId),
  toggleSolo: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>
  ) => store.getState().toggleSolo(effectsId, deviceId, configurationId),
  updateEq: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    bands: EqBandUpdate
  ) => store.getState().updateEq(effectsId, deviceId, configurationId, bands),
  updateCompressor: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    settings: CompressorUpdate
  ) => store.getState().updateCompressor(effectsId, deviceId, configurationId, settings),
  updateLimiter: (
    effectsId: Uuid<AudioEffectsDefault>,
    deviceId: Uuid<ConfiguredAudioDevice>,
    configurationId: Uuid<AudioMixerConfiguration>,
    settings: LimiterUpdate
  ) => store.getState().updateLimiter(effectsId, deviceId, configurationId, settings),
  getEffectsByDeviceId: (deviceId: Uuid<ConfiguredAudioDevice>) =>
    store.getState().getEffectsByDeviceId(deviceId),
};
