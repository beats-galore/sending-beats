import { useCallback, useMemo } from 'react';

import { useMixerStore } from '../../../stores/mixer-store';
import { useQueueStore } from '../../../stores/queue-store';
import type { FilePlayer } from '../../../types/file-player.types';
import type { Uuid } from '../../../types/util.types';

/**
 * The value the source select uses to mean "make me one".
 *
 * A queue has to exist before a channel can be patched to it, and the place you
 * decide you want one is the same place you say what feeds the channel. So the
 * select carries an entry that creates one, rather than there being a separate
 * button somewhere else that adds a source you then have to find.
 */
export const NEW_PLAYER_VALUE = 'new-queue-player';

/**
 * The queues a channel could be fed by, and the means of making another.
 *
 * Read from the studio's catalogue rather than from what happens to be running:
 * a queue built on the queues screen is available to patch straight away, and
 * one already feeding another channel is left off — two channels reading one
 * queue would each take half its audio, which reads as a broken player rather
 * than a routing mistake.
 */
export const usePlayerSources = (patchedIdentifier: string | null) => {
  const queues = useQueueStore((state) => state.queues);
  const createQueue = useQueueStore((state) => state.add);
  const addTarget = useQueueStore((state) => state.addTarget);
  const activeSession = useMixerStore((state) => state.activeSession);

  const takenElsewhere = useMemo(() => {
    const patched = (activeSession?.configuredDevices ?? [])
      .filter((device) => device.isInput)
      .map((device) => String(device.deviceIdentifier));

    return patched.filter((identifier) => identifier !== patchedIdentifier);
  }, [activeSession, patchedIdentifier]);

  const players = useMemo(
    (): FilePlayer[] => queues.map((queue) => ({ id: queue.id, name: queue.name })),
    [queues]
  );

  const options = useMemo(
    () => [
      ...players
        .filter((player) => !takenElsewhere.includes(String(player.id)))
        .map((player) => ({ value: String(player.id), label: player.name })),
      { value: NEW_PLAYER_VALUE, label: '+ New queue' },
    ],
    [players, takenElsewhere]
  );

  /** Make a queue and hand back the identifier a channel is patched to it by. */
  const create = useCallback(async (): Promise<string | null> => {
    const id = await createQueue();
    if (!id) {
      return null;
    }

    await addTarget(id);
    return String(id);
  }, [createQueue, addTarget]);

  /**
   * Say that this patch is using a queue.
   *
   * Patching a channel to it is not enough on its own — the patch has to record
   * that it wants the queue, or nothing brings it back on the next launch.
   */
  const ensureOnPatch = useCallback(
    async (identifier: string) => {
      const queue = queues.find((entry) => String(entry.id) === identifier);
      if (queue) {
        await addTarget(queue.id as Uuid<FilePlayer>);
      }
    },
    [queues, addTarget]
  );

  return { players, options, create, ensureOnPatch };
};
