// Queues as a collection, apart from whichever patch is loaded.
//
// The file player service next door drives one queue that is patched in and
// playing. This is about the catalogue: what queues exist, what is in them,
// what they have played, and which of them this patch has on its canvas.
import { invoke } from '@tauri-apps/api/core';

import type {
  FilePlayer,
  Queue,
  QueuePlay,
  QueueTrack,
} from '../types/file-player.types';
import type { Uuid } from '../types/util.types';

export const queueService = {
  /** Every queue in the studio, by name */
  async list(): Promise<Queue[]> {
    return invoke<Queue[]>('list_queues');
  },

  /**
   * What is in a queue, in the order it plays.
   *
   * Read from the database rather than the running player, so a queue can be
   * looked at and edited without being patched into anything.
   */
  async tracks(playerId: Uuid<FilePlayer>): Promise<QueueTrack[]> {
    return invoke<QueueTrack[]>('queue_tracks', { playerId });
  },

  /** What it has played, most recent first */
  async plays(playerId: Uuid<FilePlayer>): Promise<QueuePlay[]> {
    return invoke<QueuePlay[]>('queue_plays', { playerId });
  },

  /** Forget what it has played, leaving the queue itself alone */
  async clearPlays(playerId: Uuid<FilePlayer>): Promise<void> {
    return invoke('clear_queue_plays', { playerId });
  },

  async rename(playerId: Uuid<FilePlayer>, name: string): Promise<void> {
    return invoke('rename_queue', { playerId, name });
  },

  /** The queues on the current patch, by id */
  async listTargets(): Promise<Uuid<FilePlayer>[]> {
    return invoke<Uuid<FilePlayer>[]>('list_queue_targets');
  },

  /** Put a queue on the current patch so it can be patched into a channel */
  async addTarget(playerId: Uuid<FilePlayer>): Promise<void> {
    return invoke('add_queue_target', { playerId });
  },

  /** Take it off again. The queue and everything in it stay. */
  async removeTarget(playerId: Uuid<FilePlayer>): Promise<void> {
    return invoke('remove_queue_target', { playerId });
  },
} as const;
