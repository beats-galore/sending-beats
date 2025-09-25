// Zustand store for mixer state management
import isEqual from 'fast-deep-equal';
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

import { invoke } from '@tauri-apps/api/core';

import { mixerService, audioService } from '../services';
import { MixerState, DEFAULT_CHANNEL } from '../types';
import { updateArrayItems } from '../utils/store-helpers';

import type {
  MixerConfig,
  AudioChannel,
  AudioMetrics,
  MasterLevels,
  ChannelLevels,
  ChannelUpdate,
} from '../types';
import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';

type MixerStore = {
  // State
  config: MixerConfig | null;
  state: MixerState;
  error: string | null;
  metrics: AudioMetrics | null;
  masterLevels: MasterLevels;
  channelLevels: ChannelLevels;

  // Configuration Management State
  reusableConfigurations: AudioMixerConfiguration[];
  activeSession: AudioMixerConfiguration | null;
  isLoadingConfigurations: boolean;
  configurationError: string | null;

  // Actions
  initializeMixer: () => Promise<void>;
  addChannel: () => Promise<void>;
  updateChannel: (channelId: number, updates: ChannelUpdate) => Promise<void>;
  updateMasterGain: (gain: number) => Promise<void>;
  updateMasterOutputDevice: (deviceId: string) => Promise<void>;

  // Configuration Management Actions
  loadConfigurations: () => Promise<void>;
  selectConfiguration: (configId: string) => Promise<void>;
  saveSessionToReusable: () => Promise<void>;
  saveSessionAsNewReusable: (name: string, description?: string) => Promise<void>;
  clearConfigurationError: () => void;
  setConfigurationError: (error: string) => void;

  // Real-time data updates
  updateChannelLevels: (levels: Record<number, [number, number, number, number]>) => void;
  updateMasterLevels: (levels: MasterLevels) => void;
  updateMetrics: (metrics: AudioMetrics) => void;
  batchUpdate: (updates: {
    channelLevels?: ChannelLevels;
    masterLevels?: MasterLevels;
    metrics?: AudioMetrics;
  }) => void;

  // Error handling
  setError: (error: string | null) => void;
  clearError: () => void;
};

export const useMixerStore = create<MixerStore>()(
  subscribeWithSelector((set, get) => ({
    // Initial state
    config: null,
    state: MixerState.STOPPED,
    error: null,
    metrics: null,
    masterLevels: {
      left: { peak_level: 0, rms_level: 0 },
      right: { peak_level: 0, rms_level: 0 },
    },
    channelLevels: {},

    // Configuration Management Initial State
    reusableConfigurations: [],
    activeSession: null,
    isLoadingConfigurations: false,
    configurationError: null,

    // Initialize mixer (now automatically starts - always-running mode)
    initializeMixer: async () => {
      console.debug('🎛️ Initializing always-running mixer...');
      try {
        set({ state: MixerState.STARTING, error: null });

        // Get DJ-optimized configuration
        console.debug('📋 Getting DJ mixer config...');
        const djConfig = await mixerService.getDjMixerConfig();
        console.debug('📋 DJ Config loaded:', {
          channels: djConfig.channels.length,
          sampleRate: djConfig.sample_rate,
          bufferSize: djConfig.buffer_size,
        });

        console.debug('✅ Mixer created and started automatically');

        set({
          config: djConfig,
          state: MixerState.RUNNING, // Always running after creation
          error: null,
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        console.error('❌ Failed to initialize mixer:', errorMessage);
        set({
          state: MixerState.ERROR,
          error: `Failed to initialize mixer: ${errorMessage}`,
        });
        throw error;
      }
    },

    // Start/stop mixer actions removed - mixer is now always running after initialization

    // Add new channel
    addChannel: async () => {
      const { config } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      try {
        const newChannelId = config.channels.length + 1;
        const newChannel: AudioChannel = {
          ...DEFAULT_CHANNEL,
          id: newChannelId,
          name: `Channel ${newChannelId}`,
        };

        await mixerService.addMixerChannel(newChannel);

        set((state) => ({
          config: state.config
            ? {
                ...state.config,
                channels: [...state.config.channels, newChannel],
              }
            : null,
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ error: `Failed to add channel: ${errorMessage}` });
        throw error;
      }
    },

    // Update channel (with input stream management like original)
    updateChannel: async (channelId: number, updates: ChannelUpdate) => {
      const { config } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      try {
        const channelIndex = config.channels.findIndex((c) => c.id === channelId);
        if (channelIndex === -1) {
          throw new Error(`Channel ${channelId} not found`);
        }

        const previousChannel = config.channels[channelIndex];
        const updatedChannel = { ...previousChannel, ...updates };

        // Get previous and new input device IDs for stream management
        const previousInputDeviceId = previousChannel.input_device_id;
        const newInputDeviceId = updatedChannel.input_device_id;

        // Update channel configuration first
        console.log(
          `🔧 FRONTEND STORE: About to call mixerService.updateMixerChannel(${channelId}, channel with device_id: ${updatedChannel.input_device_id})`
        );

        console.log(`✅ FRONTEND STORE: Successfully called mixerService.updateMixerChannel`);

        // Handle input stream management with crash-safe switching
        if (newInputDeviceId !== previousInputDeviceId && newInputDeviceId) {
          console.debug(
            `🎤 Switching input stream: ${previousInputDeviceId} → ${newInputDeviceId}`
          );
          try {
            await audioService.switchInputStream(previousInputDeviceId ?? null, newInputDeviceId);
            console.debug(`✅ Successfully switched input stream to: ${newInputDeviceId}`);
          } catch (streamErr) {
            console.error(`❌ Failed to switch input stream to ${newInputDeviceId}:`, streamErr);
            throw new Error(`Failed to switch input stream: ${streamErr}`);
          }
        }

        // Update local state
        set((state) => ({
          config: state.config
            ? {
                ...state.config,
                channels: state.config.channels.map((channel) =>
                  channel.id === channelId ? updatedChannel : channel
                ),
              }
            : null,
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ error: `Failed to update channel: ${errorMessage}` });
        throw error;
      }
    },

    // Update master gain
    updateMasterGain: async (gain: number) => {
      const { config } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      set((state) => ({
        config: state.config
          ? {
              ...state.config,
              master_gain: gain,
            }
          : null,
      }));
    },

    // Update master output device
    updateMasterOutputDevice: async (deviceId: string) => {
      const { config } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      try {
        // Update backend first
        await audioService.setOutputStream(deviceId);

        // Update local state if backend call succeeds
        set((state) => ({
          config: state.config
            ? {
                ...state.config,
                master_output_device_id: deviceId,
              }
            : null,
        }));
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ error: `Failed to set output device: ${errorMessage}` });
        throw error;
      }
    },

    // Real-time level updates - optimized to prevent unnecessary re-renders (updated for stereo)
    updateChannelLevels: (levels: Record<number, [number, number, number, number]>) => {
      set((state) => {
        if (!state.config) return {};

        const newChannels = updateArrayItems(state.config.channels, (channel) => {
          // Stereo levels: [peak_left, rms_left, peak_right, rms_right]
          const newPeakLeft = levels[channel.id]?.[0] || 0;
          const newRmsLeft = levels[channel.id]?.[1] || 0;
          const newPeakRight = levels[channel.id]?.[2] || 0;
          const newRmsRight = levels[channel.id]?.[3] || 0;

          // For mono compatibility, use max of L/R for peak and average for RMS
          const newPeak = Math.max(newPeakLeft, newPeakRight);
          const newRms = (newRmsLeft + newRmsRight) / 2;

          if (channel.peak_level !== newPeak || channel.rms_level !== newRms) {
            return {
              ...channel,
              peak_level: newPeak,
              rms_level: newRms,
              // Store stereo data for future use
              peak_left: newPeakLeft,
              rms_left: newRmsLeft,
              peak_right: newPeakRight,
              rms_right: newRmsRight,
            };
          }
          return channel;
        });

        // Only update if channels array changed
        if (newChannels === state.config.channels) return {};

        return {
          config: {
            ...state.config,
            channels: newChannels,
          },
          channelLevels: { ...state.channelLevels, ...levels },
        };
      });
    },

    updateMasterLevels: (levels: MasterLevels) => {
      set((state) => {
        // Only update if levels actually changed
        if (isEqual(state.masterLevels, levels)) return {};
        return { masterLevels: levels };
      });
    },

    updateMetrics: (metrics: AudioMetrics) => {
      set((state) => {
        // Only update if metrics actually changed
        if (isEqual(state.metrics, metrics)) return {};
        return { metrics };
      });
    },

    // Error handling
    setError: (error: string | null) => {
      set({ error });
    },

    clearError: () => {
      set({ error: null });
    },

    // Batch update for efficient VU meter updates
    batchUpdate: (updates) => {
      set((state) => {
        const newState: Partial<MixerStore> = {};

        if (updates.channelLevels && !isEqual(state.channelLevels, updates.channelLevels)) {
          newState.channelLevels = { ...state.channelLevels, ...updates.channelLevels };
        }

        if (updates.masterLevels && !isEqual(state.masterLevels, updates.masterLevels)) {
          newState.masterLevels = updates.masterLevels;
        }

        if (updates.metrics && !isEqual(state.metrics, updates.metrics)) {
          newState.metrics = updates.metrics;
        }

        return Object.keys(newState).length > 0 ? newState : {};
      });
    },

    // Configuration Management Actions
    // Load both reusable configurations and active session
    loadConfigurations: async () => {
      set({ isLoadingConfigurations: true, configurationError: null });

      try {
        const [reusable, active] = await Promise.all([
          invoke<AudioMixerConfiguration[]>('get_reusable_configurations'),
          invoke<AudioMixerConfiguration | null>('get_active_session_configuration'),
        ]);

        set({
          reusableConfigurations: reusable,
          activeSession: active,
          isLoadingConfigurations: false,
        });
      } catch (error) {
        set({
          configurationError: error instanceof Error ? error.message : 'Failed to load configurations',
          isLoadingConfigurations: false,
        });
      }
    },

    // Select a reusable configuration and create new session
    selectConfiguration: async (configId: string) => {
      set({ isLoadingConfigurations: true, configurationError: null });

      try {
        const newSession = await invoke<AudioMixerConfiguration>('create_session_from_reusable', {
          reusableId: configId,
          sessionName: undefined,
        });

        set({
          activeSession: newSession,
          isLoadingConfigurations: false,
        });
      } catch (error) {
        set({
          configurationError: error instanceof Error ? error.message : 'Failed to select configuration',
          isLoadingConfigurations: false,
        });
      }
    },

    // Save current session back to its linked reusable configuration
    saveSessionToReusable: async () => {
      const { activeSession } = get();

      if (!activeSession?.reusableConfigurationId) {
        set({ configurationError: 'Active session is not linked to a reusable configuration' });
        return;
      }

      set({ isLoadingConfigurations: true, configurationError: null });

      try {
        await invoke('save_session_to_reusable');

        // Reload configurations to get updated data
        await get().loadConfigurations();

        set({ isLoadingConfigurations: false });
      } catch (error) {
        set({
          configurationError: error instanceof Error ? error.message : 'Failed to save configuration',
          isLoadingConfigurations: false,
        });
      }
    },

    // Save current session as a new reusable configuration
    saveSessionAsNewReusable: async (name: string, description?: string) => {
      set({ isLoadingConfigurations: true, configurationError: null });

      try {
        const newReusable = await invoke<AudioMixerConfiguration>('save_session_as_new_reusable', {
          name,
          description: description || undefined,
        });

        // Reload configurations to include the new one and get updated session
        await get().loadConfigurations();

        set({ isLoadingConfigurations: false });
      } catch (error) {
        set({
          configurationError: error instanceof Error ? error.message : 'Failed to save new configuration',
          isLoadingConfigurations: false,
        });
      }
    },

    // Configuration error handling
    clearConfigurationError: () => set({ configurationError: null }),
    setConfigurationError: (error: string) => set({ configurationError: error }),
  }))
);

// Export selector hook for configuration management
export const useConfigurationStore = () => {
  const store = useMixerStore();
  return {
    reusableConfigurations: store.reusableConfigurations,
    activeSession: store.activeSession,
    isLoading: store.isLoadingConfigurations,
    error: store.configurationError,
    loadConfigurations: store.loadConfigurations,
    selectConfiguration: store.selectConfiguration,
    saveSessionToReusable: store.saveSessionToReusable,
    saveSessionAsNewReusable: store.saveSessionAsNewReusable,
    clearError: store.clearConfigurationError,
    setError: store.setConfigurationError,
  };
};
