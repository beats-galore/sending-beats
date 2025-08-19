// Zustand store for mixer state management
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import { 
  MixerConfig, 
  AudioChannel, 
  AudioMetrics, 
  MasterLevels, 
  MixerState, 
  DEFAULT_CHANNEL,
  ChannelUpdate 
} from '../types';
import { mixerService } from '../services';

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
      right: { peak_level: 0, rms_level: 0 }
    },

    // Initialize mixer
    initializeMixer: async () => {
      try {
        set({ state: MixerState.STARTING, error: null });
        
        // Get DJ-optimized configuration
        const djConfig = await mixerService.getDjMixerConfig();
        
        // Create mixer with config
        const result = await mixerService.safeCreateMixer(djConfig);
        
        if (!result.success) {
          throw new Error(result.error || 'Failed to create mixer');
        }
        
        set({ 
          config: djConfig,
          state: MixerState.STOPPED,
          error: null 
        });
        
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ 
          state: MixerState.ERROR,
          error: `Failed to initialize mixer: ${errorMessage}`
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
          error: null 
        });
        
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ 
          state: MixerState.ERROR,
          error: `Failed to start mixer: ${errorMessage}`
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
          error: null 
        });
        
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ 
          state: MixerState.ERROR,
          error: `Failed to stop mixer: ${errorMessage}`
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
          name: `Channel ${newChannelId}`
        };

        await mixerService.addMixerChannel(newChannel);

        set(state => ({
          config: state.config ? {
            ...state.config,
            channels: [...state.config.channels, newChannel]
          } : null
        }));

      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ error: `Failed to add channel: ${errorMessage}` });
        throw error;
      }
    },

    // Update channel
    updateChannel: async (channelId: number, updates: ChannelUpdate) => {
      const { config } = get();
      if (!config) {
        throw new Error('Mixer not initialized');
      }

      try {
        const channelIndex = config.channels.findIndex(c => c.id === channelId);
        if (channelIndex === -1) {
          throw new Error(`Channel ${channelId} not found`);
        }

        const updatedChannel = { ...config.channels[channelIndex], ...updates };
        
        await mixerService.updateMixerChannel(channelId, updatedChannel);

        set(state => ({
          config: state.config ? {
            ...state.config,
            channels: state.config.channels.map(channel =>
              channel.id === channelId ? updatedChannel : channel
            )
          } : null
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

      set(state => ({
        config: state.config ? {
          ...state.config,
          master_gain: gain
        } : null
      }));
    },

    // Real-time level updates
    updateChannelLevels: (levels: Record<number, [number, number]>) => {
      set(state => ({
        config: state.config ? {
          ...state.config,
          channels: state.config.channels.map(channel => ({
            ...channel,
            peak_level: levels[channel.id]?.[0] || 0,
            rms_level: levels[channel.id]?.[1] || 0
          }))
        } : null
      }));
    },

    updateMasterLevels: (levels: MasterLevels) => {
      set({ masterLevels: levels });
    },

    updateMetrics: (metrics: AudioMetrics) => {
      set({ metrics });
    },

    // Error handling
    setError: (error: string | null) => {
      set({ error });
    },

    clearError: () => {
      set({ error: null });
    },

    // Batch updates for performance
    batchUpdate: (updates) => {
      set(state => {
        const newState: Partial<MixerStore> = {};

        if (updates.channelLevels) {
          const levels = updates.channelLevels;
          newState.config = state.config ? {
            ...state.config,
            channels: state.config.channels.map(channel => ({
              ...channel,
              peak_level: levels[channel.id]?.[0] || channel.peak_level,
              rms_level: levels[channel.id]?.[1] || channel.rms_level
            }))
          } : null;
        }

        if (updates.masterLevels) {
          newState.masterLevels = updates.masterLevels;
        }

        if (updates.metrics) {
          newState.metrics = updates.metrics;
        }

        return newState;
      });
    }
  }))
);