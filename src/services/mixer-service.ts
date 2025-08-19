// Mixer service layer - abstraction over Tauri mixer commands
import { invoke } from '@tauri-apps/api/core';

import type { MixerConfig, AudioChannel, MixerOperationResult } from '../types';

export const mixerService = {
  // Mixer lifecycle
  async getDjMixerConfig(): Promise<MixerConfig> {
    return invoke<MixerConfig>('get_dj_mixer_config');
  },

  async createMixer(config: MixerConfig): Promise<void> {
    return invoke('create_mixer', { config });
  },

  async startMixer(): Promise<void> {
    return invoke('start_mixer');
  },

  async stopMixer(): Promise<void> {
    return invoke('stop_mixer');
  },

  // Channel management
  async addMixerChannel(channel: AudioChannel): Promise<void> {
    return invoke('add_mixer_channel', { channel });
  },

  async updateMixerChannel(channelId: number, channel: AudioChannel): Promise<void> {
    return invoke('update_mixer_channel', {
      channelId,
      channel,
    });
  },

  // Higher-level operations with error handling
  async safeCreateMixer(config: MixerConfig): Promise<MixerOperationResult> {
    try {
      await this.createMixer(config);
      return { success: true };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    }
  },

  async safeStartMixer(): Promise<MixerOperationResult> {
    try {
      await this.startMixer();
      return { success: true };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    }
  },

  async safeStopMixer(): Promise<MixerOperationResult> {
    try {
      await this.stopMixer();
      return { success: true };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    }
  },
} as const;
