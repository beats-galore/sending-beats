import { useEffect, useState } from 'react';

import { trackPosition } from '../../../types/now-playing.types';
import type { NowPlayingTrack } from '../../../types/now-playing.types';

/** How often the local playhead advances while a track is playing. */
const TICK_MS = 500;

/**
 * The live playhead for a track.
 *
 * Players publish a position snapshot and only republish it when playback
 * changes, so the position is aged locally rather than asked for repeatedly.
 * A paused track has nothing to age, so no timer runs for one.
 */
export const useTrackPosition = (track: NowPlayingTrack | null): number => {
  const playing = track?.playerState === 'playing';
  const [position, setPosition] = useState(() =>
    track ? trackPosition(track, Date.now()) : 0
  );

  useEffect(() => {
    if (!track) {
      setPosition(0);
      return;
    }

    setPosition(trackPosition(track, Date.now()));

    if (!playing) {
      return;
    }

    const timer = setInterval(() => setPosition(trackPosition(track, Date.now())), TICK_MS);
    return () => clearInterval(timer);
  }, [track, playing]);

  return position;
};
