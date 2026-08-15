// Cast configuration service layer - abstraction over Tauri cast commands
import { invoke } from '@tauri-apps/api/core';

import type { CastConfiguration, CastConfigurationInput } from '../types/cast.types';

export const castConfigurationService = {
  /** Every station, by name */
  async list(): Promise<CastConfiguration[]> {
    return invoke<CastConfiguration[]>('list_cast_configurations');
  },

  async create(input: CastConfigurationInput): Promise<CastConfiguration> {
    return invoke<CastConfiguration>('create_cast_configuration', { input });
  },

  async update(id: string, input: CastConfigurationInput): Promise<CastConfiguration> {
    return invoke<CastConfiguration>('update_cast_configuration', { id, input });
  },

  /** Forget a station, and the password stored with it */
  async remove(id: string): Promise<void> {
    return invoke('delete_cast_configuration', { id });
  },

  /**
   * Store a station's password, or clear it with an empty string.
   *
   * Apart from `update` on purpose: saving a station's details never needs the
   * password, so the interface never has to hold one it is not changing.
   * Returns whether a password is now set.
   */
  async setPassword(id: string, password: string): Promise<boolean> {
    return invoke<boolean>('set_cast_configuration_password', { id, password });
  },

  /** The stations on the current patch, by id */
  async listTargets(): Promise<string[]> {
    return invoke<string[]>('list_cast_targets');
  },

  /** Put a station on the current patch so it can be routed to */
  async addTarget(castConfigurationId: string): Promise<void> {
    return invoke('add_cast_target', { castConfigurationId });
  },

  /** Take it off again, leaving its routing where it is */
  async removeTarget(castConfigurationId: string): Promise<void> {
    return invoke('remove_cast_target', { castConfigurationId });
  },
} as const;
