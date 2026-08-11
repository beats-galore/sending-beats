import { Box, Group, ScrollArea, Stack, TextInput } from '@mantine/core';
import { useCallback } from 'react';

import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { asElapsed } from '../format';
import { useStreamTransport } from '../hooks/use-stream-transport';
import { ActionButton } from '../primitives/ActionButton';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatusDot } from '../primitives/StatusDot';
import { SessionTimeline } from './SessionTimeline';
import { TrackHistory } from './TrackHistory';

/** What is on air right now, and the running log of what came before it. */
export const OnAirDrawer = () => {
  const { uptimeSeconds, controls } = useStreamTransport();

  const title = useStudioStore((state) => state.nowPlayingTitle);
  const artist = useStudioStore((state) => state.nowPlayingArtist);
  const setNowPlaying = useStudioStore((state) => state.setNowPlaying);
  const logCurrentTrack = useStudioStore((state) => state.logCurrentTrack);
  const startedAt = useStudioStore((state) => state.currentTrackStartedAt);
  const metadataPushed = useStudioStore((state) => state.metadataPushed);
  const markMetadataPushed = useStudioStore((state) => state.markMetadataPushed);
  const toggleDrawer = useStudioStore((state) => state.toggleDrawer);

  const handlePush = useCallback(() => {
    void controls.updateMetadata(title, artist).then(() => markMetadataPushed(true));
  }, [controls, title, artist, markMetadataPushed]);

  return (
    <Stack
      w={layout.shell.drawerWidth}
      gap={0}
      style={{ flex: 'none', background: color.bgRaised, borderLeft: border() }}
    >
      <Group h={38} px="xl" gap="sm" wrap="nowrap" style={{ flex: 'none', borderBottom: border() }}>
        <StatusDot tone="warn" />
        <SectionLabel tracking="section" style={{ flex: 1 }}>
          ON AIR NOW
        </SectionLabel>
        <SectionLabel tone="faint" tracking="tight">
          {asElapsed(Math.max(0, uptimeSeconds - startedAt))} in
        </SectionLabel>
        <ActionButton tone="ghost" padding="0 4px" size="md" onClick={toggleDrawer}>
          ›
        </ActionButton>
      </Group>

      <Stack gap="sm" p="xl" style={{ flex: 'none', borderBottom: border() }}>
        <TextInput
          value={title}
          onChange={(event) => {
            setNowPlaying('title', event.currentTarget.value);
            markMetadataPushed(false);
          }}
          placeholder="track title"
        />
        <TextInput
          value={artist}
          onChange={(event) => {
            setNowPlaying('artist', event.currentTarget.value);
            markMetadataPushed(false);
          }}
          placeholder="artist"
        />
        <Group gap="sm" grow wrap="nowrap">
          <ActionButton
            tone="accent"
            padding="9px 0"
            onClick={() => logCurrentTrack(uptimeSeconds)}
          >
            NEXT TRACK ⏎
          </ActionButton>
          <ActionButton tone="ghost" padding="9px 0" onClick={handlePush}>
            {metadataPushed ? 'PUSHED ✓' : 'PUSH TO SERVER'}
          </ActionButton>
        </Group>
      </Stack>

      <Box p="xl" pb="md" style={{ flex: 'none' }}>
        <SessionTimeline sessionSeconds={uptimeSeconds} />
      </Box>

      <ScrollArea style={{ flex: 1, minHeight: 0 }} px="xl" pb="xl">
        <TrackHistory sessionSeconds={uptimeSeconds} />
      </ScrollArea>
    </Stack>
  );
};
