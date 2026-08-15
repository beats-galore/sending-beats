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
  DeviceRestoreFailure,
} from '../types';
import type { ConfiguredAudioDevice } from '../types/db';
import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';
import type { Identifier } from '../types/util.types';
import { describeError } from '../utils/describe-error';
import { updateArrayItems } from '../utils/store-helpers';

type MixerStore = {
  // State
  config: MixerConfig | null;
  state: MixerState;
  error: string | null;
  // Non-fatal: the output device is live but system audio was not diverted,
  // so sources are audible twice. Kept apart from `error`, which blanks the mixer.
  systemAudioWarning: string | null;
  // The virtual audio driver was installed, which restarts coreaudiod and
  // leaves it unusable until the app is relaunched
  systemAudioRestartRequired: boolean;
  metrics: AudioMetrics | null;
  masterLevels: MasterLevels;
  channelLevels: ChannelLevels;

  // Configuration Management State
  reusableConfigurations: CompleteConfigurationData[];
  activeSession: CompleteConfigurationData | null;
  isLoadingConfigurations: boolean;
  initialConfigurationLoadingComplete: boolean;
  configurationError: string | null;
  // Devices the session lists but which could not be reconnected. Distinct from
  // `configurationError`: the session itself loaded fine, individual devices did
  // not, and the mixer stays usable without them.
  deviceRestoreFailures: DeviceRestoreFailure[];
  restoringDevicesForSession: string | null; // session ID currently being restored
  // Session IDs whose devices are currently registered in the audio pipeline. Held
  // here rather than on activeSession, which every backend refetch replaces
  // wholesale — a flag stored there is silently lost and devices get restored
  // (and re-registered) again. Reset when a session teardown clears the pipeline.
  restoredSessionIds: string[];

  // Actions
  initializeMixer: () => Promise<void>;
  addChannel: () => Promise<void>;
  /**
   * Drop a channel from the mix, unpatching whatever it was fed by.
   *
   * The last channel is emptied rather than removed, so the patch canvas is
   * never left with nothing to route.
   */
  removeChannel: (channelId: number) => Promise<void>;
  updateChannel: (channelId: number, updates: ChannelUpdate) => Promise<void>;
  updateMasterGain: (gain: number) => Promise<void>;
  /** Name a channel, or pass an empty string to clear it back to its device name */
  renameChannel: (channelId: number, name: string) => Promise<void>;
  updateMasterOutputDevice: (deviceId: Identifier<ConfiguredAudioDevice>) => Promise<void>;
  /**
   * Re-point an existing destination at a different output device.
   *
   * Resolves to an error message when the switch fails, rather than setting
   * `error` — a destination that cannot take the new device is a recoverable
   * problem with one node, not a reason to replace the mixer with an error page.
   */
  changeOutputDevice: (
    oldDeviceId: Identifier<ConfiguredAudioDevice>,
    newDeviceId: Identifier<ConfiguredAudioDevice>
  ) => Promise<string | null>;

  // Configuration Management Actions
  loadConfigurations: () => Promise<void>;
  selectConfiguration: (configId: string) => Promise<void>;
  saveSessionToReusable: () => Promise<void>;
  saveSessionAsNewReusable: (name: string, description?: string) => Promise<void>;
  clearConfigurationError: () => void;
  setConfigurationError: (error: string) => void;
  clearDeviceRestoreFailures: () => void;
  restoreDevicesFromSession: (sessionConfig: CompleteConfigurationData | null) => Promise<void>;
  updateConfiguredDevice: (device: ConfiguredAudioDevice) => void;
  removeConfiguredDevice: (deviceIdentifier: string) => void;

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
  clearSystemAudioWarning: () => void;
};

export const useMixerStore = create<MixerStore>()(
  subscribeWithSelector((set, get) => ({
    config: null,
    state: MixerState.STOPPED,
    error: null,
    systemAudioWarning: null,
    systemAudioRestartRequired: false,
    metrics: null,
    masterLevels: {
      left: { peak_level: 0, rms_level: 0 },
      right: { peak_level: 0, rms_level: 0 },
    },
    channelLevels: {},

    reusableConfigurations: [],
    activeSession: null,
    isLoadingConfigurations: false,
    initialConfigurationLoadingComplete: false,
    configurationError: null,
    deviceRestoreFailures: [],
    restoringDevicesForSession: null,
    restoredSessionIds: [],

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
        // Counting is not enough once a channel can be removed: deleting from
        // the middle would make the next add collide with an existing id.
        const newChannelId = config.channels.reduce((highest, c) => Math.max(highest, c.id), 0) + 1;
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

    removeChannel: async (channelId: number) => {
      const { config, activeSession } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      const patched = activeSession?.configuredDevices.find(
        (device) => device.channelNumber === channelId && device.isInput
      );

      if (patched) {
        try {
          await audioService.removeInputStream(patched.deviceIdentifier);
        } catch (error) {
          set({ error: `Failed to remove channel input: ${describeError(error)}` });
          throw error;
        }
        get().removeConfiguredDevice(patched.deviceIdentifier);
      }

      set((state) => {
        if (!state.config) {
          return state;
        }

        // Emptying beats removing when it is the only channel left: the patch
        // canvas would otherwise have no source to route.
        const isLast = state.config.channels.length <= 1;
        const channels = isLast
          ? state.config.channels.map((channel) =>
              channel.id === channelId
                ? { ...DEFAULT_CHANNEL, id: channel.id, name: `Channel ${channel.id}` }
                : channel
            )
          : state.config.channels.filter((channel) => channel.id !== channelId);

        return { config: { ...state.config, channels } };
      });
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

        // The channel's own configuration is not pushed to the backend: there is
        // no `update_mixer_channel` command. Only the input stream switch below
        // reaches the pipeline; everything else lives in this store.

        // Handle input stream management with crash-safe switching
        if (newInputDeviceId !== previousInputDeviceId && newInputDeviceId) {
          console.debug(
            `🎤 Switching input stream: ${previousInputDeviceId} → ${newInputDeviceId}`
          );
          try {
            await audioService.switchInputStream(previousInputDeviceId ?? null, newInputDeviceId);
            console.debug(`✅ Successfully switched input stream to: ${newInputDeviceId}`);

            // **FIX**: Refetch active session to update configuredDevices list in UI
            const updatedSession = await invoke<CompleteConfigurationData | null>(
              'get_active_session_configuration'
            );
            set({ activeSession: updatedSession });
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

    renameChannel: async (channelId: number, name: string) => {
      const trimmed = name.trim();
      await mixerService.renameChannel(channelId, trimmed);

      set((state) => ({
        config: state.config
          ? {
              ...state.config,
              channels: state.config.channels.map((channel) =>
                channel.id === channelId ? { ...channel, name: trimmed } : channel
              ),
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
        const switchResult = await audioService.setOutputStream(null, deviceId);

        // Refetch active session to update configuredDevices list in UI
        const updatedSession = await invoke<CompleteConfigurationData | null>(
          'get_active_session_configuration'
        );

        // Update local state if backend call succeeds
        set((state) => ({
          config: state.config
            ? {
                ...state.config,
                master_output_device_id: deviceId,
              }
            : null,
          activeSession: updatedSession,
          systemAudioWarning: switchResult.systemAudioDiverted ? null : switchResult.diversionError,
          systemAudioRestartRequired: switchResult.restartRequired,
          // This device just connected, so the startup failure no longer describes it
          deviceRestoreFailures: state.deviceRestoreFailures.filter(
            (failure) => failure.deviceIdentifier !== deviceId
          ),
        }));
      } catch (error) {
        set({ error: `Failed to set output device: ${describeError(error)}` });
        throw error;
      }
    },

    changeOutputDevice: async (
      oldDeviceId: Identifier<ConfiguredAudioDevice>,
      newDeviceId: Identifier<ConfiguredAudioDevice>
    ) => {
      if (oldDeviceId === newDeviceId) {
        return null;
      }

      try {
        const switchResult = await audioService.setOutputStream(oldDeviceId, newDeviceId);

        const updatedSession = await invoke<CompleteConfigurationData | null>(
          'get_active_session_configuration'
        );

        set((state) => ({
          config:
            state.config && state.config.master_output_device_id === oldDeviceId
              ? { ...state.config, master_output_device_id: newDeviceId }
              : state.config,
          activeSession: updatedSession,
          systemAudioWarning: switchResult.systemAudioDiverted ? null : switchResult.diversionError,
          systemAudioRestartRequired: switchResult.restartRequired,
          // Neither identifier describes a failed restore any more: the old device
          // is gone, and the new one just connected.
          deviceRestoreFailures: state.deviceRestoreFailures.filter(
            (failure) =>
              failure.deviceIdentifier !== oldDeviceId && failure.deviceIdentifier !== newDeviceId
          ),
        }));

        return null;
      } catch (error) {
        // The old device is torn down before the new one is attached, so a
        // failure here leaves the destination pointing at neither. Resync so the
        // patchbay shows what the pipeline actually holds rather than a device
        // that is no longer registered.
        try {
          const recoveredSession = await invoke<CompleteConfigurationData | null>(
            'get_active_session_configuration'
          );
          set({ activeSession: recoveredSession });
        } catch (resyncError) {
          console.error('Failed to resync session after output switch failure:', resyncError);
        }

        // Deliberately not `error`: that blanks the whole mixer, and a
        // destination refusing one device leaves the rest of the rig working.
        return describeError(error);
      }
    },

    // Real-time level updates - optimized to prevent unnecessary re-renders (updated for stereo)
    updateChannelLevels: (levels: Record<number, [number, number, number, number]>) => {
      set((state) => {
        if (!state.config) {
          return {};
        }

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
        if (newChannels === state.config.channels) {
          return {};
        }

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
        if (isEqual(state.masterLevels, levels)) {
          return {};
        }
        return { masterLevels: levels };
      });
    },

    updateMetrics: (metrics: AudioMetrics) => {
      set((state) => {
        // Only update if metrics actually changed
        if (isEqual(state.metrics, metrics)) {
          return {};
        }
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

    clearSystemAudioWarning: () => {
      set({ systemAudioWarning: null });
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
      const { isLoadingConfigurations, initialConfigurationLoadingComplete } = get();
      if (isLoadingConfigurations || initialConfigurationLoadingComplete) {
        return;
      }
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
                initialConfigurationLoadingComplete: true,
              });

              // Device restoration will be handled after the configuration loading is complete
            } catch (sessionError) {
              console.error('Failed to auto-select default configuration:', sessionError);
              // Fall back to just loading the configurations without active session
              set({
                reusableConfigurations: reusable,
                activeSession: active,
                isLoadingConfigurations: false,
                initialConfigurationLoadingComplete: true,
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
          !currentState.restoredSessionIds.includes(sessionId) &&
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
        // Tear down the outgoing session's devices before the new session
        // registers its own. Leaving them registered makes the incoming
        // restoration collide with device IDs that are already in the pipeline.
        await audioService.clearSessionDevices();

        // Nothing is registered any more, so no session counts as restored.
        // Without this, switching back to a previous session would skip its
        // restoration and leave it with no devices. master_output_device_id is
        // cleared for the same reason: updateMasterOutputDevice treats a matching
        // ID as a no-op, which would refuse to re-register the torn-down device.
        set((state) => ({
          restoredSessionIds: [],
          restoringDevicesForSession: null,
          config: state.config ? { ...state.config, master_output_device_id: undefined } : null,
        }));

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

        // Devices are restored by loadConfigurations, which now sees an
        // unrestored session ID and a pipeline with nothing registered
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
        await invoke<AudioMixerConfiguration>('save_session_as_new_reusable', {
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
    clearDeviceRestoreFailures: () => set({ deviceRestoreFailures: [] }),

    // Restore devices from session configuration (call existing device management methods)
    restoreDevicesFromSession: async (sessionConfig: CompleteConfigurationData | null) => {
      const sessionId = sessionConfig?.configuration.id;

      // Check if already restoring/restored this session
      const currentState = get();
      if (
        !sessionId ||
        currentState.restoringDevicesForSession === sessionId ||
        currentState.restoredSessionIds.includes(sessionId)
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
      set({ restoringDevicesForSession: sessionId, deviceRestoreFailures: [] });

      console.log(
        `🔄 Restoring ${sessionConfig.configuredDevices.length} devices from session: ${sessionConfig.configuration.name}`,
        sessionConfig
      );

      try {
        // A device that fails to reconnect must not abort the rest of the
        // restore, but it cannot be swallowed either: the session still lists it,
        // so the mixer would show a source that carries no audio.
        const failures: DeviceRestoreFailure[] = [];

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
              await audioService.setOutputStream(null, device.deviceIdentifier);
            }

            console.log(`✅ Successfully restored device: ${device.deviceIdentifier}`);
          } catch (deviceError) {
            console.error(`❌ Failed to restore device ${device.deviceIdentifier}:`, deviceError);
            failures.push({
              deviceIdentifier: device.deviceIdentifier,
              deviceName: device.deviceName ?? null,
              isInput: device.isInput,
              reason: deviceError instanceof Error ? deviceError.message : String(deviceError),
            });
          }

          // Small delay between device additions to prevent overwhelming the system
          await new Promise((resolve) => setTimeout(resolve, 100));
        }

        console.log('🎉 Device restoration completed');

        // Mark devices as restored and clear loading state
        set((state) => ({
          restoredSessionIds: state.restoredSessionIds.includes(sessionId)
            ? state.restoredSessionIds
            : [...state.restoredSessionIds, sessionId],
          restoringDevicesForSession: null,
          deviceRestoreFailures: failures,
        }));
      } catch (error) {
        console.error('❌ Failed to restore devices from session:', error);
        set({
          configurationError: error instanceof Error ? error.message : 'Failed to restore devices',
          restoringDevicesForSession: null, // Clear loading state on error
        });
      }
    },

    updateConfiguredDevice: (device: ConfiguredAudioDevice) => {
      set((state) => {
        if (!state.activeSession) {
          return state;
        }

        // Remove any existing device with the same deviceIdentifier and isInput/channelNumber
        // Then add the new device
        const filteredDevices = state.activeSession.configuredDevices.filter(
          (d) => !(d.deviceIdentifier === device.deviceIdentifier && d.isInput === device.isInput)
        );

        const updatedDevices = [...filteredDevices, device];

        console.log('📝 updateConfiguredDevice:', {
          deviceId: device.id,
          deviceIdentifier: device.deviceIdentifier,
          channelNumber: device.channelNumber,
          before: state.activeSession.configuredDevices.length,
          after: updatedDevices.length,
        });

        return {
          activeSession: {
            ...state.activeSession,
            configuredDevices: updatedDevices,
          },
          // This device just connected, so the startup failure no longer describes it
          deviceRestoreFailures: state.deviceRestoreFailures.filter(
            (failure) => failure.deviceIdentifier !== device.deviceIdentifier
          ),
        };
      });
    },

    removeConfiguredDevice: (deviceIdentifier: string) => {
      set((state) => {
        if (!state.activeSession) {
          return state;
        }

        const updatedDevices = state.activeSession.configuredDevices.filter(
          (d) => d.deviceIdentifier !== deviceIdentifier
        );

        console.log('🗑️ removeConfiguredDevice:', {
          deviceIdentifier,
          before: state.activeSession.configuredDevices.length,
          after: updatedDevices.length,
        });

        return {
          activeSession: {
            ...state.activeSession,
            configuredDevices: updatedDevices,
          },
        };
      });
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
    updateConfiguredDevice: store.updateConfiguredDevice,
    removeConfiguredDevice: store.removeConfiguredDevice,
  };
};
