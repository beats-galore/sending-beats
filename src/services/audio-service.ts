// Audio service layer - abstraction over Tauri audio commands
import { invoke } from '@tauri-apps/api/core';

import type { AudioDeviceInfo, AudioMetrics } from '../types';

export const audioService = {
  // Device management
  async enumerateAudioDevices(): Promise<AudioDeviceInfo[]> {
    return invoke<AudioDeviceInfo[]>('enumerate_audio_devices');
  },

  async refreshAudioDevices(): Promise<AudioDeviceInfo[]> {
    return invoke<AudioDeviceInfo[]>('refresh_audio_devices');
  },

  // Real-time data
  async getMixerMetrics(): Promise<AudioMetrics> {
    return invoke<AudioMetrics>('get_mixer_metrics');
  },

  async getChannelLevels(): Promise<Record<number, [number, number]>> {
    return invoke<Record<number, [number, number]>>('get_channel_levels');
  },

  async getMasterLevels(): Promise<[number, number, number, number]> {
    return invoke<[number, number, number, number]>('get_master_levels');
  },

  // Stream management
  async addInputStream(deviceId: string): Promise<void> {
    return invoke('add_input_stream', { deviceId });
  },

  async removeInputStream(deviceId: string): Promise<void> {
    return invoke('remove_input_stream', { deviceId });
  },

  async setOutputStream(deviceId: string): Promise<void> {
    return invoke('set_output_stream', { deviceId });
  },
} as const;
