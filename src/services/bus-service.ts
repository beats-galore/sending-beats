// Bus routing service layer - abstraction over Tauri bus commands
import { invoke } from '@tauri-apps/api/core';

import type { Bus } from '../types/bus.types';

export const busService = {
  /** Every bus, with the inputs feeding it and the outputs taking it */
  async list(): Promise<Bus[]> {
    return invoke<Bus[]>('list_audio_buses');
  },

  /**
   * Lay the session's stored routing over the devices already registered.
   *
   * Devices join the main bus as they register, so without this a saved patch
   * comes back as one shared mix however it was routed when it was saved.
   */
  async restore(): Promise<Bus[]> {
    return invoke<Bus[]>('restore_audio_buses');
  },

  /**
   * Point a destination at exactly the inputs it should receive.
   *
   * Returns the whole routing table rather than the one destination: putting
   * two destinations on the same inputs merges them onto one bus, so an edit
   * here can change what another destination is on.
   */
  async setOutputSources(deviceId: string, inputIds: string[]): Promise<Bus[]> {
    return invoke<Bus[]>('set_output_sources', { deviceId, inputIds });
  },

  /** Trim one mix. `gain` is a linear multiplier, not dB. */
  async setGain(busId: string, gain: number): Promise<void> {
    return invoke('set_audio_bus_gain', { busId, gain });
  },
} as const;
