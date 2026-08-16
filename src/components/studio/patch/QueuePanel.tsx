import { Box, Group, Stack, Text } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import type { FilePlayer } from '../../../types/file-player.types';
import type { Uuid } from '../../../types/util.types';
import { asTrackTime } from '../format';
import { useFilePlayer } from '../hooks/use-file-player';
import type { NodeRect } from './patch-layout';
import { QueueBreakNote } from './QueueBreakNote';
import { QueueDrop } from './QueueDrop';
import { QueueList } from './QueueList';
import { QueueNowPlaying } from './QueueNowPlaying';

type QueuePanelProps = {
  playerId: Uuid<FilePlayer>;
  /** Box of the source node this belongs to: the panel stands beside it. */
  anchor: NodeRect;
  /** The player's colour, taken from the source it is patched into. */
  tint: string;
};

const PANEL_WIDTH = 316;
/** Between the source card and the panel. */
const PANEL_GAP = 16;
/** Shortest the panel gets, so a shrunk source still leaves a usable queue. */
const MIN_HEIGHT = 420;

/**
 * The queue, beside the player it belongs to.
 *
 * A panel rather than part of the card: a queue is a list that wants room, and
 * a source card that grew to hold one would push everything below it down the
 * canvas every time you looked at what was coming up. It floats over the mix
 * column instead, and closes when the player is deselected.
 */
export const QueuePanel = ({ playerId, anchor, tint }: QueuePanelProps) => {
  const player = useFilePlayer(playerId);
  const { queue, status, playing, currentIndex, actions, toggle, total } = player;

  const track = currentIndex === null ? null : (queue[currentIndex] ?? null);

  return (
    <Box
      // Clicks inside are for the queue. Letting them reach the canvas would
      // clear the selection and close the panel out from under the pointer.
      onClick={(event) => event.stopPropagation()}
      style={{
        position: 'absolute',
        left: anchor.left + anchor.width + PANEL_GAP,
        top: anchor.top,
        width: PANEL_WIDTH,
        height: Math.max(anchor.height, MIN_HEIGHT),
        display: 'flex',
        flexDirection: 'column',
        background: color.panel,
        border: border('lineStrong'),
        borderRadius: 'var(--mantine-radius-md)',
        boxShadow: 'var(--mantine-shadow-xl)',
        zIndex: 60,
      }}
    >
      <Group
        gap="sm"
        wrap="nowrap"
        style={{
          flex: 'none',
          height: 34,
          padding: '0 12px',
          borderBottom: border(),
          background: color.panelHi,
          borderRadius: 'var(--mantine-radius-md) var(--mantine-radius-md) 0 0',
        }}
      >
        <Text
          size="sm"
          fw={700}
          truncate
          style={{ flex: 1, minWidth: 0, letterSpacing: layout.tracking.wider }}
        >
          QUEUE · {player.name ?? 'PLAYER'}
        </Text>
        <Text size="3xs" c={color.textFaint} style={{ letterSpacing: layout.tracking.wide }}>
          {queue.length} {queue.length === 1 ? 'TRACK' : 'TRACKS'} · {asTrackTime(total / 1000)}
        </Text>
      </Group>

      <Stack gap="sm" p="md" style={{ flex: 'none', borderBottom: border() }}>
        <QueueNowPlaying
          track={track}
          playing={playing}
          position={status?.position ?? 0}
          tint={tint}
          onToggle={toggle}
          onPrevious={actions.previous}
          onNext={actions.next}
          onStop={actions.stop}
          onSeek={actions.seek}
        />
        <QueueBreakNote
          tracks={queue}
          currentIndex={currentIndex}
          position={status?.position ?? 0}
          breakpointTrackId={status?.breakpointTrackId ?? null}
          tint={tint}
          onClear={() => actions.breakAfter(null)}
        />
      </Stack>

      <QueueList
        tracks={queue}
        currentIndex={currentIndex}
        playing={playing}
        breakpointTrackId={status?.breakpointTrackId ?? null}
        tint={tint}
        onPlayNow={actions.playTrack}
        onMove={actions.move}
        onRemove={actions.remove}
        onBreakAfter={actions.breakAfter}
      />

      <QueueDrop tint={tint} onDrop={actions.add} />
    </Box>
  );
};
