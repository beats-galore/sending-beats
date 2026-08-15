import { Box, Group, Stack, Text } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import type { NowPlayingTrack } from '../../../types/now-playing.types';
import { asTrackTime } from '../format';
import { useTrackPosition } from '../hooks/use-track-position';

type ChannelTrackProps = {
  /** Null while the application is running but has nothing loaded. */
  track: NowPlayingTrack | null;
};

/** What the patched application is playing, read above its meters. */
export const ChannelTrack = ({ track }: ChannelTrackProps) => {
  const position = useTrackPosition(track);

  if (!track) {
    return (
      <Text size="2xs" c={color.textFaintest} truncate>
        Nothing playing
      </Text>
    );
  }

  const playing = track.playerState === 'playing';
  // A live stream reports no length, which would make any progress meaningless.
  const progress =
    track.durationSeconds > 0
      ? Math.min(1, Math.max(0, position / track.durationSeconds))
      : 0;

  return (
    <Stack gap={layout.source.trackProgressGap} style={{ minWidth: 0 }}>
      <Stack gap={0} style={{ minWidth: 0 }}>
        <Text
          size="2xs"
          c={playing ? color.acc : color.textDim}
          truncate
          title={`${track.title} — ${track.album}`}
        >
          {playing ? '♪' : '⏸'} {track.title}
        </Text>
        <Group gap="xs" wrap="nowrap" justify="space-between">
          <Text size="3xs" c={color.textFaint} truncate>
            {track.artist}
          </Text>
          <Text size="3xs" c={color.textFaintest} style={{ flex: 'none' }}>
            {asTrackTime(position)} / {asTrackTime(track.durationSeconds)}
          </Text>
        </Group>
      </Stack>

      <Box
        style={{
          height: layout.source.trackProgressHeight,
          background: color.bg,
          borderRadius: 'var(--mantine-radius-2xs)',
          overflow: 'hidden',
        }}
      >
        <Box
          style={{
            width: `${progress * 100}%`,
            height: '100%',
            background: playing ? color.playback : color.playbackDim,
            // Matches the poll beat, so the bar creeps rather than steps.
            transition: 'width 500ms linear',
          }}
        />
      </Box>
    </Stack>
  );
};
