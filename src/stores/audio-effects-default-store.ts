import { invoke } from '@tauri-apps/api/core';
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

import type { AudioEffectsDefault } from '../types/db/audio-effects.types';
import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';
import type { ConfiguredAudioDevice } from '../types/db/configured-audio-devices.types';
import type { Uuid } from '../types/util.types';

type AudioEffectsDefaultStore = {
  effectsById: Record<string, AudioEffectsDefault>;
  isLoading: boolean;
  error: string | null;
  loadedConfigurationId: string | null;

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

  getEffectsByDeviceId: (deviceId: Uuid<ConfiguredAudioDevice>) => AudioEffectsDefault | null;
  setError: (error: string | null) => void;
  clearError: () => void;
};

const store = create<AudioEffectsDefaultStore>()(
  subscribeWithSelector((set, get) => ({
    effectsById: {},
    isLoading: false,
    error: null,
    loadedConfigurationId: null,

    loadEffects: async (configurationId: Uuid<AudioMixerConfiguration>) => {
      const { isLoading, loadedConfigurationId } = get();

      // Skip if already loading or already loaded for this configuration
      if (isLoading || loadedConfigurationId === configurationId) {
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

        set({ effectsById, isLoading: false, loadedConfigurationId: configurationId });
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
  getEffectsByDeviceId: (deviceId: Uuid<ConfiguredAudioDevice>) =>
    store.getState().getEffectsByDeviceId(deviceId),
};