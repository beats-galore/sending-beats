// Audio service layer - abstraction over Tauri audio commands
import { invoke } from '@tauri-apps/api/core';

import type { AudioDeviceInfo, AudioMetrics, OutputDevice, DeviceHealth } from '../types';

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

  async getChannelLevels(): Promise<Record<number, [number, number, number, number]>> {
    return invoke<Record<number, [number, number, number, number]>>('get_channel_levels');
  },

  async getMasterLevels(): Promise<[number, number, number, number]> {
    return invoke<[number, number, number, number]>('get_master_levels');
  },

  async removeInputStream(deviceId: string): Promise<void> {
    return invoke('remove_input_stream', { deviceId });
  },

  async switchInputStream(oldDeviceId: string | null, newDeviceId: string): Promise<void> {
    return invoke('safe_switch_input_device', { oldDeviceId, newDeviceId });
  },

  async setOutputStream(deviceId: string): Promise<void> {
    return invoke('safe_switch_output_device', { newDeviceId: deviceId });
  },

  // Effects management
  async addChannelEffect(channelId: number, effectType: string): Promise<void> {
    return invoke('add_channel_effect', { channelId, effectType });
  },

  async removeChannelEffect(channelId: number, effectType: string): Promise<void> {
    return invoke('remove_channel_effect', { channelId, effectType });
  },

  async getChannelEffects(channelId: number): Promise<string[]> {
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

  async removeOutputDevice(deviceId: string): Promise<void> {
    return invoke('remove_output_device', { deviceId });
  },

  async updateOutputDevice(
    deviceId: string,
    options: {
      deviceName?: string;
      gain?: number;
      enabled?: boolean;
      isMonitor?: boolean;
    }
  ): Promise<void> {
    return invoke('update_output_device', {
      deviceId,
      deviceName: options.deviceName,
      gain: options.gain,
      enabled: options.enabled,
      isMonitor: options.isMonitor,
    });
  },

  async getOutputDevices(): Promise<OutputDevice[]> {
    return invoke('get_output_devices');
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
} as const;
