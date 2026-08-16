// File player service layer - abstraction over the Tauri file player commands
import { invoke } from '@tauri-apps/api/core';

import type {
  FilePlayer,
  FilePlayerConfig,
  PlaybackAction,
  PlaybackStatus,
  QueuedTrack,
} from '../types/file-player.types';
import { PLAYER_CHANNELS, PLAYER_SAMPLE_RATE } from '../types/file-player.types';
import type { FilePath, Uuid } from '../types/util.types';

/** What a player is created with when nothing else is asked for. */
const defaultConfig = (name: string): FilePlayerConfig => ({
  name,
  sampleRate: PLAYER_SAMPLE_RATE,
  channels: PLAYER_CHANNELS,
  autoPlayNext: true,
  volume: 1,
});

export const filePlayerService = {
  /** Create a player and return the id a channel is patched to it by */
  async create(name: string): Promise<Uuid<FilePlayer>> {
    return invoke<Uuid<FilePlayer>>('create_file_player', { config: defaultConfig(name) });
  },

  async remove(playerId: Uuid<FilePlayer>): Promise<void> {
    return invoke('remove_file_player', { playerId });
  },

  /**
   * Rebuild this session's players and their queues
   *
   * Called before the channels patched to them are restored: a player has to
   * exist before a channel can be attached to it.
   */
  async restore(): Promise<Uuid<FilePlayer>[]> {
    return invoke<Uuid<FilePlayer>[]>('restore_file_players');
  },

  /** Every player currently running */
  async list(): Promise<FilePlayer[]> {
    const running = await invoke<[Uuid<FilePlayer>, string][]>('list_file_players');
    return running.map(([id, name]) => ({ id, name }));
  },

  async queue(playerId: Uuid<FilePlayer>): Promise<QueuedTrack[]> {
    return invoke<QueuedTrack[]>('get_player_queue', { playerId });
  },

  async status(playerId: Uuid<FilePlayer>): Promise<PlaybackStatus> {
    return invoke<PlaybackStatus>('get_player_status', { playerId });
  },

  /** Put a file on the end of the queue, returning the id it was queued under */
  async addTrack(playerId: Uuid<FilePlayer>, filePath: FilePath): Promise<Uuid<QueuedTrack>> {
    return invoke<Uuid<QueuedTrack>>('add_track_to_player', { playerId, filePath });
  },

  async removeTrack(playerId: Uuid<FilePlayer>, trackId: Uuid<QueuedTrack>): Promise<void> {
    return invoke('remove_track_from_player', { playerId, trackId });
  },

  /** Move a track to a new place in the queue */
  async moveTrack(
    playerId: Uuid<FilePlayer>,
    trackId: Uuid<QueuedTrack>,
    toIndex: number
  ): Promise<void> {
    return invoke('move_track_in_player', { playerId, trackId, toIndex });
  },

  /** Empty the queue, leaving what the player already played */
  async clearQueue(playerId: Uuid<FilePlayer>): Promise<void> {
    return invoke('clear_player_queue', { playerId });
  },

  /** Pause after a track, or pass null to stop pausing anywhere */
  async setBreakpoint(
    playerId: Uuid<FilePlayer>,
    trackId: Uuid<QueuedTrack> | null
  ): Promise<void> {
    return invoke('set_player_breakpoint', { playerId, trackId });
  },

  async control(playerId: Uuid<FilePlayer>, action: PlaybackAction): Promise<void> {
    return invoke('control_file_player', { playerId, action });
  },
} as const;
