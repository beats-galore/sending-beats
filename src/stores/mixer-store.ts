// Zustand store for mixer state management
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import isEqual from 'fast-deep-equal';

import { mixerService, audioService } from '../services';
import { MixerState, DEFAULT_CHANNEL } from '../types';
import { updateArrayItems, hasLevelChanges } from '../utils/store-helpers';

import type {
  MixerConfig,
  AudioChannel,
  AudioMetrics,
  MasterLevels,
  ChannelUpdate,
} from '../types';

type MixerStore = {
  // State
  config: MixerConfig | null;
  state: MixerState;
  error: string | null;
  metrics: AudioMetrics | null;
  masterLevels: MasterLevels;

  // Actions
  initializeMixer: () => Promise<void>;
  startMixer: () => Promise<void>;
  stopMixer: () => Promise<void>;
  addChannel: () => Promise<void>;
  updateChannel: (channelId: number, updates: ChannelUpdate) => Promise<void>;
  updateMasterGain: (gain: number) => Promise<void>;
  updateMasterOutputDevice: (deviceId: string) => Promise<void>;

  // Real-time data updates
  updateChannelLevels: (levels: Record<number, [number, number]>) => void;
  updateMasterLevels: (levels: MasterLevels) => void;
  updateMetrics: (metrics: AudioMetrics) => void;

  // Error handling
  setError: (error: string | null) => void;
  clearError: () => void;

  // Batch updates for performance
  batchUpdate: (updates: {
    channelLevels?: Record<number, [number, number]>;
    masterLevels?: MasterLevels;
    metrics?: AudioMetrics;
  }) => void;
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

    // Initialize mixer
    initializeMixer: async () => {
      console.debug('🎛️ Initializing mixer...');
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

        // Create mixer with config
        console.debug('🔧 Creating mixer...');
        const result = await mixerService.safeCreateMixer(djConfig);

        if (!result.success) {
          throw new Error(result.error || 'Failed to create mixer');
        }
        console.debug('✅ Mixer created successfully');

        set({
          config: djConfig,
          state: MixerState.STOPPED,
          error: null,
        });
        console.debug('🎛️ Mixer initialized successfully');
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

    // Start mixer
    startMixer: async () => {
      try {
        set({ state: MixerState.STARTING, error: null });

        const result = await mixerService.safeStartMixer();

        if (!result.success) {
          throw new Error(result.error || 'Failed to start mixer');
        }

        set({
          state: MixerState.RUNNING,
          error: null,
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({
          state: MixerState.ERROR,
          error: `Failed to start mixer: ${errorMessage}`,
        });
        throw error;
      }
    },

    // Stop mixer
    stopMixer: async () => {
      try {
        set({ state: MixerState.STOPPING, error: null });

        const result = await mixerService.safeStopMixer();

        if (!result.success) {
          throw new Error(result.error || 'Failed to stop mixer');
        }

        set({
          state: MixerState.STOPPED,
          error: null,
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({
          state: MixerState.ERROR,
          error: `Failed to stop mixer: ${errorMessage}`,
        });
        throw error;
      }
    },

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
        await mixerService.updateMixerChannel(channelId, updatedChannel);

        // Handle input stream management (critical missing functionality)
        if (newInputDeviceId && newInputDeviceId !== previousInputDeviceId) {
          console.debug(`🎤 Adding input stream for device: ${newInputDeviceId}`);
          try {
            await audioService.addInputStream(newInputDeviceId);
            console.debug(`✅ Successfully added input stream for: ${newInputDeviceId}`);
          } catch (streamErr) {
            console.error(`❌ Failed to add input stream for ${newInputDeviceId}:`, streamErr);
            throw new Error(`Failed to add input stream: ${streamErr}`);
          }
        }

        // If input device was removed, remove input stream
        if (previousInputDeviceId && !newInputDeviceId) {
          console.debug(`🗑️ Removing input stream for device: ${previousInputDeviceId}`);
          try {
            await audioService.removeInputStream(previousInputDeviceId);
            console.debug(`✅ Successfully removed input stream for: ${previousInputDeviceId}`);
          } catch (streamErr) {
            console.error(
              `❌ Failed to remove input stream for ${previousInputDeviceId}:`,
              streamErr
            );
            // Don't throw here - removal failure shouldn't block the update
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

    // Real-time level updates - optimized to prevent unnecessary re-renders
    updateChannelLevels: (levels: Record<number, [number, number]>) => {
      set((state) => {
        if (!state.config) return {};

        const newChannels = updateArrayItems(state.config.channels, (channel) => {
          const newPeak = levels[channel.id]?.[0] || 0;
          const newRms = levels[channel.id]?.[1] || 0;

          if (channel.peak_level !== newPeak || channel.rms_level !== newRms) {
            return {
              ...channel,
              peak_level: newPeak,
              rms_level: newRms,
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

    // Batch updates for performance - optimized to prevent unnecessary re-renders
    batchUpdate: (updates) => {
      set((state) => {
        const newState: Partial<MixerStore> = {};

        // Handle channel level updates
        if (updates.channelLevels && state.config) {
          const levels = updates.channelLevels;
          const newChannels = updateArrayItems(state.config.channels, (channel) => {
            const newPeak = levels[channel.id]?.[0] || channel.peak_level;
            const newRms = levels[channel.id]?.[1] || channel.rms_level;

            if (channel.peak_level !== newPeak || channel.rms_level !== newRms) {
              return {
                ...channel,
                peak_level: newPeak,
                rms_level: newRms,
              };
            }
            return channel;
          });

          // Only update config if channels array changed
          if (newChannels !== state.config.channels) {
            newState.config = {
              ...state.config,
              channels: newChannels,
            };
          }
        }

        // Handle master levels with deep equality check
        if (updates.masterLevels && !isEqual(state.masterLevels, updates.masterLevels)) {
          newState.masterLevels = updates.masterLevels;
        }

        // Handle metrics with deep equality check
        if (updates.metrics && !isEqual(state.metrics, updates.metrics)) {
          newState.metrics = updates.metrics;
        }

        // Only return changes if there are any
        return Object.keys(newState).length > 0 ? newState : {};
      });
    },
  }))
);
