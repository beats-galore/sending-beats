import { Box, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import type { QueuedTrack } from '../../../types/file-player.types';
import type { Uuid } from '../../../types/util.types';
import { QueueBreakSeam } from './QueueBreakSeam';
import { QueueTrackRow } from './QueueTrackRow';

type QueueListProps = {
  tracks: QueuedTrack[];
  /** Which row is playing, or null with nothing loaded. */
  currentIndex: number | null;
  playing: boolean;
  /** The track the player pauses after, when one is set. */
  breakpointTrackId: Uuid<QueuedTrack> | null;
  tint: string;
  onPlayNow: (trackId: Uuid<QueuedTrack>) => void;
  onMove: (trackId: Uuid<QueuedTrack>, toIndex: number) => void;
  onRemove: (trackId: Uuid<QueuedTrack>) => void;
  onBreakAfter: (trackId: Uuid<QueuedTrack> | null) => void;
};

/** Everything waiting to play, in the order it will. */
export const QueueList = ({
  tracks,
  currentIndex,
  playing,
  breakpointTrackId,
  tint,
  onPlayNow,
  onMove,
  onRemove,
  onBreakAfter,
}: QueueListProps) => {
  if (tracks.length === 0) {
    return (
      <Box style={{ flex: 1, minHeight: 0, display: 'grid', placeItems: 'center', padding: 16 }}>
        <Text size="2xs" c={color.textFaintest} ta="center">
          Nothing queued. Drop audio files below to build one.
        </Text>
      </Box>
    );
  }

  const breakIndex = tracks.findIndex((track) => track.id === breakpointTrackId);

  return (
    <Box
      style={{
        flex: 1,
        minHeight: 0,
        overflowY: 'auto',
        padding: 8,
        display: 'flex',
        flexDirection: 'column',
        gap: 2,
      }}
    >
      {tracks.map((track, index) => (
        <Box key={track.id} style={{ display: 'contents' }}>
          <QueueTrackRow
            track={track}
            index={index}
            current={index === currentIndex}
            playing={playing}
            afterBreak={breakIndex !== -1 && index > breakIndex}
            tint={tint}
            onPlayNow={() => onPlayNow(track.id)}
            onMoveUp={() => onMove(track.id, index - 1)}
            onMoveDown={() => onMove(track.id, index + 1)}
            onRemove={() => onRemove(track.id)}
          />
          {/* No seam below the last track: there is nothing after it to pause
              before, and the queue running out is not a break. */}
          {index < tracks.length - 1 && (
            <QueueBreakSeam
              index={index}
              broken={index === breakIndex}
              tint={tint}
              onToggle={() => onBreakAfter(index === breakIndex ? null : track.id)}
            />
          )}
        </Box>
      ))}
    </Box>
  );
};
