import { Box, Group, Stack, Text } from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { border, color } from '../../../theme/tokens';
import { asElapsed } from '../format';

type TrackHistoryProps = {
  sessionSeconds: number;
};

/** The session's tracks, most recent first, with the live one flagged. */
export const TrackHistory = ({ sessionSeconds }: TrackHistoryProps) => {
  const trackLog = useStudioStore((state) => state.trackLog);
  const title = useStudioStore((state) => state.nowPlayingTitle);
  const artist = useStudioStore((state) => state.nowPlayingArtist);
  const startedAt = useStudioStore((state) => state.currentTrackStartedAt);

  const entries = [
    ...trackLog.map((track) => ({ ...track, live: false })),
    {
      title: title || 'Untitled track',
      artist,
      startedAt,
      durationSeconds: Math.max(1, sessionSeconds - startedAt),
      live: true,
    },
  ].reverse();

  return (
    <Stack gap={0}>
      {entries.map((entry, index) => (
        <Stack key={`${entry.title}-${index}`} gap="3xs" py="md" style={{ borderBottom: border() }}>
          <Group gap="sm" align="baseline" wrap="nowrap">
            <Text size="2xs" c={color.textFaintest} w={44} style={{ flex: 'none' }}>
              {asElapsed(entry.startedAt)}
            </Text>
            <Text size="sm" truncate c={entry.live ? color.warn : color.text} style={{ flex: 1 }}>
              {entry.live ? `▶ ${entry.title}` : entry.title}
            </Text>
            <Text size="2xs" c={color.textFaint} style={{ flex: 'none' }}>
              {asElapsed(entry.durationSeconds)}
            </Text>
          </Group>
          <Box pl={53}>
            <Text size="2xs" c={color.textDim} truncate>
              {entry.artist}
            </Text>
          </Box>
        </Stack>
      ))}
    </Stack>
  );
};
