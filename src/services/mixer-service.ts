// Mixer service layer - abstraction over Tauri mixer commands
import { invoke } from '@tauri-apps/api/core';

import type { MixerConfig, AudioChannel } from '../types';

export const mixerService = {
  // Mixer lifecycle
  async getDjMixerConfig(): Promise<MixerConfig> {
    return invoke<MixerConfig>('get_dj_mixer_config');
  },

  // Channel management
  //
  // NOTE: `add_mixer_channel` is not registered in the Rust invoke_handler, so
  // this call currently rejects. Adding a channel still updates the interface
  // and the session, but the pipeline never learns about it.
  async addMixerChannel(channel: AudioChannel): Promise<void> {
    return invoke('add_mixer_channel', { channel });
  },

  async requestAudioCapturePermissions(): Promise<string> {
    return invoke<string>('request_audio_capture_permissions');
  },
} as const;
