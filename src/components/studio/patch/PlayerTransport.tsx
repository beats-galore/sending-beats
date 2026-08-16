import { Box, Group, Text } from '@mantine/core';

import { border, color } from '../../../theme/tokens';
import { trackTitle } from '../../../types/file-player.types';
import { asTrackTime } from '../format';
import type { useFilePlayer } from '../hooks/use-file-player';
import { PlayButton } from '../primitives/PlayButton';
import { ScrubBar } from '../primitives/ScrubBar';

type PlayerTransportProps = {
  player: ReturnType<typeof useFilePlayer>;
  /** The strip's own colour, which the playhead and the button read in. */
  tint: string;
};

/**
 * What the player is on, above its meters.
 *
 * The card's answer to a track readout: an application card reports what
 * something else is playing, where this one is playing it, so the same row
 * carries the transport rather than only the reading.
 */
export const PlayerTransport = ({ player, tint }: PlayerTransportProps) => {
  const { status, queue, currentIndex, playing, toggle, actions } = player;
  const track = currentIndex === null ? null : queue[currentIndex];
  const trackNumber = String((currentIndex ?? 0) + 1).padStart(2, '0');

  const position = (status?.position ?? 0) / 1000;
  const length = (track?.duration ?? 0) / 1000;

  return (
    <Group
      gap="xs"
      wrap="nowrap"
      onClick={(event) => event.stopPropagation()}
      style={{
        padding: '3px 8px 3px 3px',
        border: border(),
        borderRadius: 'var(--mantine-radius-sm)',
        background: color.bg,
      }}
    >
      <PlayButton playing={playing} tint={tint} onToggle={toggle} disabled={queue.length === 0} />

      <Box style={{ flex: 1, minWidth: 0 }}>
        <Text size="2xs" c={track ? color.text : color.textFaintest} truncate>
          {track ? `${trackNumber} · ${trackTitle(track)}` : 'queue empty — drop files in'}
        </Text>
        <Box mt={4}>
          <ScrubBar position={position} length={length} tint={tint} onSeek={actions.seek} />
        </Box>
      </Box>

      <Text size="2xs" c={color.textDim} style={{ flex: 'none' }}>
        {asTrackTime(position)} / {asTrackTime(length)}
      </Text>
    </Group>
  );
};
