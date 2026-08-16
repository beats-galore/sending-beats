import { useCallback, useEffect, useMemo } from 'react';

import { useFilePlayerStore } from '../../../stores/file-player-store';
import { useQueueStore } from '../../../stores/queue-store';
import type {
  FilePlayer,
  PlaybackAction,
  PlaybackStatus,
  QueuedTrack,
} from '../../../types/file-player.types';
import { queueDuration } from '../../../types/file-player.types';
import type { FilePath, Uuid } from '../../../types/util.types';

/**
 * How often the playhead is read back.
 *
 * A player has no clock to publish from — its position moves as audio leaves it
 * — so unlike an application, which announces a track and a start time once,
 * this has to be asked. Half a second is finer than the readout shows and
 * cheap enough for the two or three players a session ever has.
 */
const POLL_MS = 500;

/** Stable empties, so a player with nothing queued does not re-render on every read. */
const NO_TRACKS: QueuedTrack[] = [];

/**
 * Load this session's players and keep their playheads current.
 *
 * Mounted once by the canvas rather than per node: the poll is per player, not
 * per card, and two cards watching one player would double it.
 */
export const useFilePlayers = (): void => {
  const load = useFilePlayerStore((state) => state.load);
  const poll = useFilePlayerStore((state) => state.poll);
  const players = useFilePlayerStore((state) => state.players);
  const loadQueues = useQueueStore((state) => state.load);

  useEffect(() => {
    void load();
    // The catalogue as well as what is running: the dock offers every queue in
    // the studio, not only the ones this patch already has.
    void loadQueues();
  }, [load, loadQueues]);

  useEffect(() => {
    if (players.length === 0) {
      return;
    }

    const timer = setInterval(() => void poll(), POLL_MS);
    return () => clearInterval(timer);
  }, [poll, players.length]);
};

/**
 * One player's queue, what it is doing, and the controls for it.
 *
 * Takes a nullable id so a source card can call it whether or not a player is
 * what feeds it. With none, everything reads empty and every control is inert,
 * which is what a card patched to a microphone needs.
 */
export const useFilePlayer = (playerId: Uuid<FilePlayer> | null) => {
  const queue = useFilePlayerStore((state) =>
    playerId ? (state.queues[playerId] ?? NO_TRACKS) : NO_TRACKS
  );
  const status = useFilePlayerStore((state) =>
    playerId ? (state.statuses[playerId] ?? null) : null
  );
  const name = useFilePlayerStore(
    (state) => state.players.find((player) => player.id === playerId)?.name ?? null
  );

  const control = useFilePlayerStore((state) => state.control);
  const addTracks = useFilePlayerStore((state) => state.addTracks);
  const removeTrack = useFilePlayerStore((state) => state.removeTrack);
  const moveTrack = useFilePlayerStore((state) => state.moveTrack);
  const clearQueue = useFilePlayerStore((state) => state.clearQueue);
  const browseForTracks = useFilePlayerStore((state) => state.browseForTracks);
  const setBreakpoint = useFilePlayerStore((state) => state.setBreakpoint);

  const playing = status?.state === 'playing';

  const actions = useMemo(() => {
    // Every control goes through here, so "there is no player" is answered once
    // rather than at each of a dozen call sites.
    const send = (action: PlaybackAction) => {
      if (playerId) {
        void control(playerId, action);
      }
    };

    return {
      play: () => send({ type: 'play' }),
      pause: () => send({ type: 'pause' }),
      stop: () => send({ type: 'stop' }),
      next: () => send({ type: 'skipNext' }),
      previous: () => send({ type: 'skipPrevious' }),
      playTrack: (trackId: Uuid<QueuedTrack>) => send({ type: 'playTrack', trackId }),
      seek: (seconds: number) => send({ type: 'seek', seconds }),
      add: (filePaths: FilePath[]) => {
        if (playerId) {
          void addTracks(playerId, filePaths);
        }
      },
      remove: (trackId: Uuid<QueuedTrack>) => {
        if (playerId) {
          void removeTrack(playerId, trackId);
        }
      },
      move: (trackId: Uuid<QueuedTrack>, toIndex: number) => {
        if (playerId) {
          void moveTrack(playerId, trackId, toIndex);
        }
      },
      clear: () => {
        if (playerId) {
          void clearQueue(playerId);
        }
      },
      browse: () => {
        if (playerId) {
          void browseForTracks(playerId);
        }
      },
      breakAfter: (trackId: Uuid<QueuedTrack> | null) => {
        if (playerId) {
          void setBreakpoint(playerId, trackId);
        }
      },
    };
  }, [playerId, control, addTracks, removeTrack, moveTrack, clearQueue, browseForTracks, setBreakpoint]);

  const currentIndex = currentIndexOf(status, queue);

  const toggle = useCallback(() => {
    if (playing) {
      actions.pause();
      return;
    }
    actions.play();
  }, [playing, actions]);

  return {
    name,
    queue,
    status,
    playing,
    actions,
    toggle,
    /** Where the queue is, for the row that is marked as playing. */
    currentIndex,
    /**
     * The track playing, or the one pressing play would start.
     *
     * A player with a queue but nothing open yet is about to play its first
     * track, and saying "queue empty" over four queued tracks is simply false.
     */
    cuedIndex: currentIndex ?? (queue.length > 0 ? 0 : null),
    /** Total run time of everything queued, in milliseconds. */
    total: queueDuration(queue),
  };
};

/**
 * Which row is playing.
 *
 * The status carries an index, but the queue in hand may be a beat ahead of it
 * — a track just dragged, say — so the id is what settles it and the index is
 * only the fallback for a track with no row of its own.
 */
const currentIndexOf = (status: PlaybackStatus | null, queue: QueuedTrack[]): number | null => {
  if (!status?.currentTrack) {
    return null;
  }

  const found = queue.findIndex((track) => track.id === status.currentTrack?.id);
  return found === -1 ? status.currentIndex : found;
};
