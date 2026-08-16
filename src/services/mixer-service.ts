// Mixer service layer - abstraction over Tauri mixer commands
import { invoke } from '@tauri-apps/api/core';

import type { MixerConfig } from '../types';

export const mixerService = {
  // Mixer lifecycle
  async getDjMixerConfig(): Promise<MixerConfig> {
    return invoke<MixerConfig>('get_dj_mixer_config');
  },

  /** Name a channel, or pass an empty string to clear it back to its device name */
  async renameChannel(channelNumber: number, name: string): Promise<void> {
    return invoke('rename_mixer_channel', { channelNumber, name });
  },

  async requestAudioCapturePermissions(): Promise<string> {
    return invoke<string>('request_audio_capture_permissions');
  },
} as const;
