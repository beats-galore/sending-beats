// Zustand store for mixer state management
import { invoke } from '@tauri-apps/api/core';
import isEqual from 'fast-deep-equal';
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

import { mixerService, audioService } from '../services';
import { MixerState, DEFAULT_CHANNEL } from '../types';

import type {
  MixerConfig,
  AudioChannel,
  AudioMetrics,
  MasterLevels,
  ChannelLevels,
  ChannelUpdate,
  CompleteConfigurationData,
} from '../types';
import type { ConfiguredAudioDevice } from '../types/db';
import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';
import type { Identifier } from '../types/util.types';
import { updateArrayItems } from '../utils/store-helpers';

type MixerStore = {
  // State
  config: MixerConfig | null;
  state: MixerState;
  error: string | null;
  metrics: AudioMetrics | null;
  masterLevels: MasterLevels;
  channelLevels: ChannelLevels;

  // Configuration Management State
  reusableConfigurations: CompleteConfigurationData[];
  activeSession: CompleteConfigurationData | null;
  isLoadingConfigurations: boolean;
  configurationError: string | null;
  restoringDevicesForSession: string | null; // session ID currently being restored

  // Actions
  initializeMixer: () => Promise<void>;
  addChannel: () => Promise<void>;
  updateChannel: (channelId: number, updates: ChannelUpdate) => Promise<void>;
  updateMasterGain: (gain: number) => Promise<void>;
  updateMasterOutputDevice: (deviceId: Identifier<ConfiguredAudioDevice>) => Promise<void>;

  // Configuration Management Actions
  loadConfigurations: () => Promise<void>;
  selectConfiguration: (configId: string) => Promise<void>;
  saveSessionToReusable: () => Promise<void>;
  saveSessionAsNewReusable: (name: string, description?: string) => Promise<void>;
  clearConfigurationError: () => void;
  setConfigurationError: (error: string) => void;
  restoreDevicesFromSession: (sessionConfig: CompleteConfigurationData | null) => Promise<void>;

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
    config: null,
    state: MixerState.STOPPED,
    error: null,
    metrics: null,
    masterLevels: {
      left: { peak_level: 0, rms_level: 0 },
      right: { peak_level: 0, rms_level: 0 },
    },
    channelLevels: {},

    reusableConfigurations: [],
    activeSession: null,
    isLoadingConfigurations: false,
    configurationError: null,
    restoringDevicesForSession: null,

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
          state: MixerState.RUNNING,
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
    updateMasterGain: async (gainDb: number) => {
      const { config } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      // Convert dB to linear gain for audio pipeline
      const linearGain = 10 ** (gainDb / 20);

      // Update audio pipeline with linear gain
      await invoke('update_master_gain', { gain: linearGain });

      // Store dB value in local state
      set((state) => ({
        config: state.config
          ? {
              ...state.config,
              master_gain: gainDb,
            }
          : null,
      }));
    },

    // Update master output device
    updateMasterOutputDevice: async (deviceId: Identifier<ConfiguredAudioDevice>) => {
      const { config } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      try {
        // Check if already using this output device - no-op to prevent unnecessary stream restart
        if (config.master_output_device_id === deviceId) {
          console.debug(`📋 Output device no-op: already using device ${deviceId}`);
          return;
        }

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
        if (!state.config) {return {};}

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
        if (newChannels === state.config.channels) {return {};}

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
        if (isEqual(state.masterLevels, levels)) {return {};}
        return { masterLevels: levels };
      });
    },

    updateMetrics: (metrics: AudioMetrics) => {
      set((state) => {
        // Only update if metrics actually changed
        if (isEqual(state.metrics, metrics)) {return {};}
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
          invoke<CompleteConfigurationData[]>('get_reusable_configurations'),
          invoke<CompleteConfigurationData | null>('get_active_session_configuration'),
        ]);

        // If no active session, auto-select the default configuration
        if (!active && reusable.length > 0) {
          const defaultConfig = reusable.find((config) => config.configuration.isDefault);
          if (defaultConfig) {
            console.log(
              `🔄 No active session found, auto-selecting default configuration: ${defaultConfig.configuration.name}`
            );

            // Create session from default configuration
            try {
              await invoke<AudioMixerConfiguration>('create_session_from_reusable', {
                reusableId: defaultConfig.configuration.id,
                sessionName: undefined,
              });

              // Fetch the newly created session's complete data
              const newSession = await invoke<CompleteConfigurationData | null>(
                'get_active_session_configuration'
              );

              set({
                reusableConfigurations: reusable,
                activeSession: newSession,
                isLoadingConfigurations: false,
              });

              // Device restoration will be handled after the configuration loading is complete
            } catch (sessionError) {
              console.error('Failed to auto-select default configuration:', sessionError);
              // Fall back to just loading the configurations without active session
              set({
                reusableConfigurations: reusable,
                activeSession: active,
                isLoadingConfigurations: false,
              });
            }
          } else {
            // No default configuration found, just load what we have
            set({
              reusableConfigurations: reusable,
              activeSession: active,
              isLoadingConfigurations: false,
            });
          }
        } else {
          // Active session exists or no reusable configs, proceed normally
          set({
            reusableConfigurations: reusable,
            activeSession: active,
            isLoadingConfigurations: false,
          });
        }

        // After successfully loading configurations, restore devices from active session
        // Only restore if not already restoring/restored for this session
        const currentState = get();
        const sessionId = currentState.activeSession?.configuration.id;
        const shouldRestore =
          sessionId &&
          currentState.activeSession?.configuredDevices?.length &&
          !currentState.activeSession.devicesRestored &&
          currentState.restoringDevicesForSession !== sessionId;

        if (shouldRestore) {
          await get().restoreDevicesFromSession(currentState.activeSession);
        }
      } catch (error) {
        set({
          configurationError:
            error instanceof Error ? error.message : 'Failed to load configurations',
          isLoadingConfigurations: false,
        });
      }
    },

    // Select a reusable configuration and create new session
    selectConfiguration: async (configId: string) => {
      set({ isLoadingConfigurations: true, configurationError: null });

      try {
        await invoke<AudioMixerConfiguration>('create_session_from_reusable', {
          reusableId: configId,
          sessionName: undefined,
        });

        // Fetch the newly created session's complete data
        const newSession = await invoke<CompleteConfigurationData | null>(
          'get_active_session_configuration'
        );

        set({
          activeSession: newSession,
          isLoadingConfigurations: false,
        });

        // Mark that devices need restoration (will be handled by loadConfigurations after this)
      } catch (error) {
        set({
          configurationError:
            error instanceof Error ? error.message : 'Failed to select configuration',
          isLoadingConfigurations: false,
        });
      }
    },

    // Save current session back to its linked reusable configuration
    saveSessionToReusable: async () => {
      const { activeSession } = get();

      if (!activeSession?.configuration?.reusableConfigurationId) {
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
          configurationError:
            error instanceof Error ? error.message : 'Failed to save configuration',
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
          description: description ?? undefined,
        });

        // Reload configurations to include the new one and get updated session
        await get().loadConfigurations();

        set({ isLoadingConfigurations: false });
      } catch (error) {
        set({
          configurationError:
            error instanceof Error ? error.message : 'Failed to save new configuration',
          isLoadingConfigurations: false,
        });
      }
    },

    // Configuration error handling
    clearConfigurationError: () => set({ configurationError: null }),
    setConfigurationError: (error: string) => set({ configurationError: error }),

    // Restore devices from session configuration (call existing device management methods)
    restoreDevicesFromSession: async (sessionConfig: CompleteConfigurationData | null) => {
      const sessionId = sessionConfig?.configuration.id;

      // Check if already restoring/restored this session
      const currentState = get();
      if (
        currentState?.restoringDevicesForSession === sessionId ||
        sessionConfig?.devicesRestored
      ) {
        console.log(
          `📋 Device restoration already in progress or completed for session: ${sessionId}`
        );
        return;
      }

      if (!sessionConfig?.configuredDevices?.length) {
        console.log('📋 No configured devices to restore in session');
        return;
      }

      // Set loading state to prevent concurrent restoration
      set({ restoringDevicesForSession: sessionId });

      console.log(
        `🔄 Restoring ${sessionConfig.configuredDevices.length} devices from session: ${sessionConfig.configuration.name}`,
        sessionConfig
      );

      try {
        // Restore devices one by one to avoid overwhelming the audio system
        for (const device of sessionConfig.configuredDevices) {
          console.log(
            `🎚️ Restoring ${device.isInput ? 'input' : 'output'} device: ${device.deviceName ?? 'Unknown'} (${device.deviceIdentifier})`
          );

          try {
            if (device.isInput) {
              // Restore input device using switchInputStream (null -> deviceId)
              await audioService.switchInputStream(null, device.deviceIdentifier);
            } else {
              // Restore output device using setOutputStream
              await audioService.setOutputStream(device.deviceIdentifier);
            }

            console.log(`✅ Successfully restored device: ${device.deviceIdentifier}`);
          } catch (deviceError) {
            console.error(`❌ Failed to restore device ${device.deviceIdentifier}:`, deviceError);
            // Continue with other devices even if one fails
          }

          // Small delay between device additions to prevent overwhelming the system
          await new Promise((resolve) => setTimeout(resolve, 100));
        }

        console.log('🎉 Device restoration completed');

        // Mark devices as restored and clear loading state
        set((state) => ({
          activeSession: state.activeSession
            ? {
                ...state.activeSession,
                devicesRestored: true,
              }
            : null,
          restoringDevicesForSession: null,
        }));
      } catch (error) {
        console.error('❌ Failed to restore devices from session:', error);
        set({
          configurationError: error instanceof Error ? error.message : 'Failed to restore devices',
          restoringDevicesForSession: null, // Clear loading state on error
        });
      }
    },
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
