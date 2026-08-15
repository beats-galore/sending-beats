// Audio service layer - abstraction over Tauri audio commands
import { invoke } from '@tauri-apps/api/core';

import type { AudioDeviceInfo, DeviceHealth, OutputDeviceSwitchResult } from '../types';
import type { ConfiguredAudioDevice } from '../types/db';
import type { Identifier } from '../types/util.types';

export const audioService = {
  // Device management
  async enumerateAudioDevices(): Promise<AudioDeviceInfo[]> {
    return invoke<AudioDeviceInfo[]>('enumerate_audio_devices');
  },

  async refreshAudioDevices(): Promise<AudioDeviceInfo[]> {
    return invoke<AudioDeviceInfo[]>('refresh_audio_devices');
  },

  // Real-time data
  async getChannelLevels(): Promise<Record<number, [number, number, number, number]>> {
    return invoke<Record<number, [number, number, number, number]>>('get_channel_levels');
  },

  async removeInputStream(deviceId: Identifier<ConfiguredAudioDevice>): Promise<void> {
    return invoke('remove_input_stream', { deviceId });
  },

  async removeOutputStream(deviceId: Identifier<ConfiguredAudioDevice>): Promise<void> {
    return invoke('remove_output_stream', { deviceId });
  },

  /**
   * Tear down every device registered by the current session.
   *
   * Resolves to the number of devices removed. Recording and Icecast output taps
   * are left running.
   */
  async clearSessionDevices(): Promise<number> {
    return invoke<number>('clear_session_devices');
  },

  async switchInputStream(
    oldDeviceId: Identifier<ConfiguredAudioDevice> | null,
    newDeviceId: Identifier<ConfiguredAudioDevice>,
    isVirtual?: boolean
  ): Promise<ConfiguredAudioDevice | null> {
    return invoke<ConfiguredAudioDevice | null>('safe_switch_input_device', {
      oldDeviceId,
      newDeviceId,
      isVirtual,
    });
  },

  /**
   * Point a destination at an output device.
   *
   * `oldDeviceId` is the device this destination currently uses, and is torn
   * down before the new one joins — passing null adds a destination instead of
   * re-pointing one.
   */
  async setOutputStream(
    oldDeviceId: Identifier<ConfiguredAudioDevice> | null,
    newDeviceId: Identifier<ConfiguredAudioDevice>
  ): Promise<OutputDeviceSwitchResult> {
    return invoke<OutputDeviceSwitchResult>('safe_switch_output_device', {
      oldDeviceId,
      newDeviceId,
    });
  },

  // Effects management
  async addChannelEffect(channelId: number, effectType: string): Promise<void> {
    return invoke('add_channel_effect', { channelId, effectType });
  },

  async removeChannelEffect(channelId: number, effectType: string): Promise<void> {
    return invoke('remove_channel_effect', { channelId, effectType });
  },

  async getChannelEffects(_channelId: number): Promise<string[]> {
    return [];
    // return invoke('get_channel_effects', { channelId });
  },

  // Enhanced effects update commands
  async updateChannelEQ(
    channelId: number,
    options: {
      eqLowGain?: number;
      eqMidGain?: number;
      eqHighGain?: number;
    }
  ): Promise<void> {
    return invoke('update_channel_eq', {
      channelId,
      eqLowGain: options.eqLowGain,
      eqMidGain: options.eqMidGain,
      eqHighGain: options.eqHighGain,
    });
  },

  async updateChannelCompressor(
    channelId: number,
    options: {
      threshold?: number;
      ratio?: number;
      attackMs?: number;
      releaseMs?: number;
      enabled?: boolean;
    }
  ): Promise<void> {
    return invoke('update_channel_compressor', {
      channelId,
      threshold: options.threshold,
      ratio: options.ratio,
      attackMs: options.attackMs,
      releaseMs: options.releaseMs,
      enabled: options.enabled,
    });
  },

  async updateChannelLimiter(
    channelId: number,
    options: {
      thresholdDb?: number;
      enabled?: boolean;
    }
  ): Promise<void> {
    return invoke('update_channel_limiter', {
      channelId,
      thresholdDb: options.thresholdDb,
      enabled: options.enabled,
    });
  },

  // Device health monitoring
  async getDeviceHealth(deviceId: string): Promise<DeviceHealth | null> {
    return invoke('get_device_health', { deviceId });
  },

  async getAllDeviceHealth(): Promise<Record<string, DeviceHealth>> {
    return invoke('get_all_device_health');
  },

  async reportDeviceError(deviceId: string, error: string): Promise<void> {
    return invoke('report_device_error', { deviceId, error });
  },

  // VU Level Events
  async initializeVuEvents(): Promise<void> {
    return invoke('initialize_vu_channels');
  },

  // VU Level Channels (high-performance streaming)
  async initializeVuChannels(): Promise<void> {
    return invoke('initialize_vu_channels');
  },
} as const;
