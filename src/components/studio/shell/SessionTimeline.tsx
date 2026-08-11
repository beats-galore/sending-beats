import { Box, Group, Stack, Tooltip } from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { color } from '../../../theme/tokens';
import { asElapsed } from '../format';
import { SectionLabel } from '../primitives/SectionLabel';

type SessionTimelineProps = {
  sessionSeconds: number;
};

/** Every track this session as one proportional bar, with the live one lit. */
export const SessionTimeline = ({ sessionSeconds }: SessionTimelineProps) => {
  const trackLog = useStudioStore((state) => state.trackLog);
  const nowPlayingTitle = useStudioStore((state) => state.nowPlayingTitle);
  const nowPlayingArtist = useStudioStore((state) => state.nowPlayingArtist);
  const currentTrackStartedAt = useStudioStore((state) => state.currentTrackStartedAt);

  const segments = [
    ...trackLog.map((track) => ({ ...track, live: false })),
    {
      title: nowPlayingTitle || 'Untitled track',
      artist: nowPlayingArtist,
      durationSeconds: Math.max(1, sessionSeconds - currentTrackStartedAt),
      live: true,
    },
  ];

  return (
    <Stack gap="sm">
      <Group gap="sm" wrap="nowrap">
        <SectionLabel tracking="section">SESSION</SectionLabel>
        <Box style={{ flex: 1, height: 1, background: color.line }} />
        <SectionLabel tone="faint" tracking="tight">
          {segments.length} tracks · {asElapsed(sessionSeconds)}
        </SectionLabel>
      </Group>

      <Group gap="3xs" wrap="nowrap" h={8}>
        {segments.map((segment, index) => (
          <Tooltip
            key={`${segment.title}-${index}`}
            label={`${segment.title} — ${segment.artist} · ${asElapsed(segment.durationSeconds)}`}
          >
            <Box
              style={{
                flex: `${Math.max(1, segment.durationSeconds)} 1 0`,
                minWidth: 0,
                height: '100%',
                borderRadius: 'var(--mantine-radius-2xs)',
                background: segment.live ? color.warn : color.accDim,
              }}
            />
          </Tooltip>
        ))}
      </Group>
    </Stack>
  );
};
