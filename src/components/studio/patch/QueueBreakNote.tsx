import { Box, Group, Text } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import type { QueuedTrack } from '../../../types/file-player.types';
import { trackTitle } from '../../../types/file-player.types';
import type { Uuid } from '../../../types/util.types';
import { asTrackTime } from '../format';
import { ActionButton } from '../primitives/ActionButton';

type QueueBreakNoteProps = {
  tracks: QueuedTrack[];
  /** Which row is playing, or null with nothing loaded. */
  currentIndex: number | null;
  /** Milliseconds into the current track. */
  position: number;
  /** The track the player pauses after, when one is set. */
  breakpointTrackId: Uuid<QueuedTrack> | null;
  tint: string;
  onClear: () => void;
};

/** Whether the player stops on its own, and how long until it does. */
export const QueueBreakNote = ({
  tracks,
  currentIndex,
  position,
  breakpointTrackId,
  tint,
  onClear,
}: QueueBreakNoteProps) => {
  const breakIndex = tracks.findIndex((track) => track.id === breakpointTrackId);
  const broken = breakIndex !== -1;

  return (
    <>
      <Group gap="xs" wrap="nowrap">
        <Box
          fz="3xs"
          style={{
            flex: 1,
            minWidth: 0,
            padding: '4px 7px',
            borderRadius: 'var(--mantine-radius-2xs)',
            letterSpacing: layout.tracking.label,
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            color: broken ? tint : color.textFaint,
            border: broken ? `1px solid ${tint}` : border(),
          }}
        >
          {broken
            ? `BREAKPOINT · pauses after ${trackTitle(tracks[breakIndex])}`
            : 'CONTINUOUS · click between tracks to set a breakpoint'}
        </Box>
        {broken && (
          <ActionButton tone="danger" onClick={onClear} padding="4px 8px" size="3xs">
            CLEAR
          </ActionButton>
        )}
      </Group>

      <Text size="3xs" c={color.textFaintest} style={{ letterSpacing: layout.tracking.tight }}>
        {remainingLabel(tracks, currentIndex, position, breakIndex)}
      </Text>
    </>
  );
};

/**
 * How much is left to play, to the break or to the end.
 *
 * Counted from where the playhead is rather than from the start of the current
 * track, because the question this answers is "how long have I got" — which is
 * the whole reason for setting a break.
 */
const remainingLabel = (
  tracks: QueuedTrack[],
  currentIndex: number | null,
  position: number,
  breakIndex: number
): string => {
  const from = currentIndex ?? 0;
  const to = breakIndex === -1 ? tracks.length : breakIndex + 1;

  if (tracks.length === 0 || to <= from) {
    return breakIndex === -1 ? 'nothing left in queue' : 'the break has passed';
  }

  const queued = tracks
    .slice(from, to)
    .reduce((total, track) => total + (track.duration ?? 0), 0);

  // A queue holding files that declare no length cannot be counted, and a
  // countdown that is quietly wrong about an ad break is worse than none.
  if (tracks.slice(from, to).some((track) => track.duration === null)) {
    return breakIndex === -1 ? 'queue length unknown' : 'time to pause unknown';
  }

  const remaining = Math.max(0, (queued - position) / 1000);
  return breakIndex === -1
    ? `${asTrackTime(remaining)} left in queue`
    : `${asTrackTime(remaining)} until pause`;
};
