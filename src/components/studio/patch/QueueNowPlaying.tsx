import { Group, Stack, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import type { QueuedTrack } from '../../../types/file-player.types';
import { trackTitle } from '../../../types/file-player.types';
import { asTrackTime } from '../format';
import { ActionButton } from '../primitives/ActionButton';
import { PlayButton } from '../primitives/PlayButton';
import { ScrubBar } from '../primitives/ScrubBar';

type QueueNowPlayingProps = {
  /** Null with nothing loaded, which is what an empty queue looks like. */
  track: QueuedTrack | null;
  playing: boolean;
  /** Milliseconds into the track. */
  position: number;
  tint: string;
  onToggle: () => void;
  onPrevious: () => void;
  onNext: () => void;
  onStop: () => void;
  onSeek: (seconds: number) => void;
};

/** What the player is on, and the transport for it. */
export const QueueNowPlaying = ({
  track,
  playing,
  position,
  tint,
  onToggle,
  onPrevious,
  onNext,
  onStop,
  onSeek,
}: QueueNowPlayingProps) => {
  const seconds = position / 1000;
  const length = (track?.duration ?? 0) / 1000;

  return (
    <Stack gap="sm">
      <Group gap="sm" wrap="nowrap">
        <PlayButton playing={playing} tint={tint} onToggle={onToggle} disabled={!track} />

        <Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
          <Text size="sm" c={track ? color.text : color.textFaint} truncate>
            {track ? trackTitle(track) : 'nothing loaded'}
          </Text>
          <Text size="2xs" c={color.textMuted} truncate>
            {track?.artist ?? 'drop audio files below to build a queue'}
          </Text>
        </Stack>

        <Group gap="3xs" wrap="nowrap" style={{ flex: 'none' }}>
          <ActionButton onClick={onPrevious} padding="4px 7px" size="3xs">
            ⏮
          </ActionButton>
          <ActionButton onClick={onNext} padding="4px 7px" size="3xs">
            ⏭
          </ActionButton>
          <ActionButton tone="danger" onClick={onStop} padding="4px 7px" size="3xs">
            ■
          </ActionButton>
        </Group>
      </Group>

      <Group gap="sm" wrap="nowrap" align="center">
        <ScrubBar
          position={seconds}
          length={length}
          tint={tint}
          height={4}
          onSeek={onSeek}
        />
        <Text size="2xs" c={color.textDim} style={{ flex: 'none' }}>
          {asTrackTime(seconds)} / {asTrackTime(length)}
        </Text>
      </Group>
    </Stack>
  );
};
