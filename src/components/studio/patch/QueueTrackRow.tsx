import { Box, Group, Stack, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import type { QueuedTrack } from '../../../types/file-player.types';
import { trackTitle } from '../../../types/file-player.types';
import { asTrackTime } from '../format';

type QueueTrackRowProps = {
  track: QueuedTrack;
  /** Position in the queue, for the number shown. */
  index: number;
  current: boolean;
  playing: boolean;
  /** Past the break, so it is not going to play this time round. */
  afterBreak: boolean;
  /** The player's colour, which the current row is marked in. */
  tint: string;
  onPlayNow: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onRemove: () => void;
};

const ROW_HEIGHT = 34;

/** One track waiting to play: what it is, how long, and where it sits. */
export const QueueTrackRow = ({
  track,
  index,
  current,
  playing,
  afterBreak,
  tint,
  onPlayNow,
  onMoveUp,
  onMoveDown,
  onRemove,
}: QueueTrackRowProps) => {
  const stateGlyph = (() => {
    if (!current) {
      return '';
    }
    return playing ? '▶' : '❙❙';
  })();

  return (
  <Group
    gap="xs"
    wrap="nowrap"
    onClick={onPlayNow}
    title={`Play ${trackTitle(track)} now`}
    style={{
      flex: 'none',
      height: ROW_HEIGHT,
      padding: '0 7px',
      borderRadius: 'var(--mantine-radius-xs)',
      border: `1px solid ${current ? tint : 'transparent'}`,
      background: current ? color.panelHi : 'transparent',
      // Dimmed rather than hidden: the break says these are not playing yet,
      // not that they have gone.
      opacity: afterBreak ? 0.55 : 1,
      cursor: 'pointer',
    }}
  >
    <Text size="3xs" c={color.textFaintest} w={14} style={{ flex: 'none' }}>
      {String(index + 1).padStart(2, '0')}
    </Text>

    <Stack gap={0} style={{ flex: 1, minWidth: 0 }}>
      <Text size="2xs" c={current ? color.text : color.textDim} fw={current ? 600 : 400} truncate>
        {trackTitle(track)}
      </Text>
      <Text size="3xs" c={color.textFaintest} truncate>
        {track.artist ?? 'unknown artist'}
      </Text>
    </Stack>

    <Text size="3xs" c={tint} w={8} style={{ flex: 'none' }}>
      {stateGlyph}
    </Text>

    <Text size="2xs" c={color.textFaint} style={{ flex: 'none' }}>
      {track.duration === null ? '--:--' : asTrackTime(track.duration / 1000)}
    </Text>

    <Stack gap={0} style={{ flex: 'none' }}>
      <Nudge label="▲" title="Move up" onClick={onMoveUp} />
      <Nudge label="▼" title="Move down" onClick={onMoveDown} />
    </Stack>

    <Box
      onClick={(event) => {
        event.stopPropagation();
        onRemove();
      }}
      title={`Remove ${trackTitle(track)} from the queue`}
      style={{
        flex: 'none',
        fontSize: 12,
        lineHeight: 1,
        color: color.textFaintest,
        cursor: 'pointer',
      }}
    >
      ×
    </Box>
  </Group>
  );
};

type NudgeProps = {
  label: string;
  title: string;
  onClick: () => void;
};

/** One half of the reorder control. Stacked, so a row moves without a drag. */
const Nudge = ({ label, title, onClick }: NudgeProps) => (
  <Box
    onClick={(event) => {
      event.stopPropagation();
      onClick();
    }}
    title={title}
    style={{
      fontSize: 7,
      lineHeight: 1.1,
      color: color.textFaintest,
      cursor: 'pointer',
    }}
  >
    {label}
  </Box>
);
