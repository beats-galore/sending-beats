import { Box, Text } from '@mantine/core';
import { Fragment } from 'react';

import { color } from '../../../theme/tokens';
import type { QueuedTrack } from '../../../types/file-player.types';
import type { Uuid } from '../../../types/util.types';
import { reorderList, useDragReorder, withDragApplied } from '../hooks/use-drag-reorder';
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
  // Before the early return: an empty queue is a thing to draw, not a reason to
  // have a different set of hooks.
  const { drag, start } = useDragReorder<QueuedTrack['id']>({ onMove });

  if (tracks.length === 0) {
    return (
      <Box style={{ flex: 1, minHeight: 0, display: 'grid', placeItems: 'center', padding: 16 }}>
        <Text size="2xs" c={color.textFaintest} ta="center">
          Nothing queued. Drop audio files below to build one.
        </Text>
      </Box>
    );
  }

  // Drawn in the order the drag would leave it, so the rows move under the
  // pointer rather than the dragged one merely lighting up.
  const shown = withDragApplied(tracks, drag);
  const breakIndex = shown.findIndex((track) => track.id === breakpointTrackId);
  const currentTrackId = currentIndex === null ? null : tracks[currentIndex]?.id;

  return (
    <Box
      {...reorderList}
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
      {shown.map((track, index) => (
        <Fragment key={track.id}>
          <QueueTrackRow
            track={track}
            index={index}
            current={track.id === currentTrackId}
            playing={playing}
            afterBreak={breakIndex !== -1 && index > breakIndex}
            tint={tint}
            dragging={drag?.id === track.id}
            onPlayNow={() => onPlayNow(track.id)}
            onGrab={(event) => start(event, track.id, index, shown.length)}
            onRemove={() => onRemove(track.id)}
          />
          {/* No seam below the last track: there is nothing after it to pause
              before, and the queue running out is not a break. */}
          {index < shown.length - 1 && (
            <QueueBreakSeam
              index={index}
              broken={index === breakIndex}
              tint={tint}
              onToggle={() => onBreakAfter(index === breakIndex ? null : track.id)}
            />
          )}
        </Fragment>
      ))}
    </Box>
  );
};
