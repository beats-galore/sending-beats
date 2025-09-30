// Mixer service layer - abstraction over Tauri mixer commands
import { invoke } from '@tauri-apps/api/core';

import type { MixerConfig, AudioChannel, MixerOperationResult } from '../types';

export const mixerService = {
  // Mixer lifecycle
  async getDjMixerConfig(): Promise<MixerConfig> {
    return invoke<MixerConfig>('get_dj_mixer_config');
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

  async requestAudioCapturePermissions(): Promise<string> {
    return invoke<string>('request_audio_capture_permissions');
  },
} as const;
