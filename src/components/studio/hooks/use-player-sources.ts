import { useCallback, useMemo } from 'react';

import { useFilePlayerStore } from '../../../stores/file-player-store';
import { useMixerStore } from '../../../stores/mixer-store';

/**
 * The value the source select uses to mean "make me one".
 *
 * A player has to exist before a channel can be patched to it, and the place
 * you decide you want one is the same place you say what feeds the channel. So
 * the select carries an entry that creates one, rather than there being a
 * separate button somewhere else that adds a source you then have to find.
 */
export const NEW_PLAYER_VALUE = 'new-queue-player';

/** What the first player is called, before there is anything to number it against. */
const FIRST_PLAYER_NAME = 'Sweet Beats Player';

/**
 * The players a channel could be fed by, and the means of making another.
 *
 * A player already feeding another channel is left off: two channels reading one
 * queue would each take half its audio, which is not a thing anyone wants and
 * reads as a broken player rather than a routing mistake.
 */
export const usePlayerSources = (patchedIdentifier: string | null) => {
  const players = useFilePlayerStore((state) => state.players);
  const createPlayer = useFilePlayerStore((state) => state.create);
  const activeSession = useMixerStore((state) => state.activeSession);

  const takenElsewhere = useMemo(() => {
    const patched = (activeSession?.configuredDevices ?? [])
      .filter((device) => device.isInput)
      .map((device) => String(device.deviceIdentifier));

    return patched.filter((identifier) => identifier !== patchedIdentifier);
  }, [activeSession, patchedIdentifier]);

  const options = useMemo(
    () => [
      ...players
        .filter((player) => !takenElsewhere.includes(player.id))
        .map((player) => ({ value: player.id, label: player.name })),
      { value: NEW_PLAYER_VALUE, label: '+ New queue player' },
    ],
    [players, takenElsewhere]
  );

  /** Make a player and hand back the identifier a channel is patched to it by. */
  const create = useCallback(async (): Promise<string | null> => {
    const name =
      players.length === 0 ? FIRST_PLAYER_NAME : `${FIRST_PLAYER_NAME} ${players.length + 1}`;

    return createPlayer(name);
  }, [createPlayer, players.length]);

  return { players, options, create };
};
