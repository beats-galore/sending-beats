import { TextInput } from '@mantine/core';
import { useCallback } from 'react';

import { useStudioStore } from '../../../stores/studio-store';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';
import { SectionLabel } from '../primitives/SectionLabel';

type NowPlayingPanelProps = {
  onPush: (title: string, artist: string) => Promise<void>;
};

/** The metadata the server shows listeners. */
export const NowPlayingPanel = ({ onPush }: NowPlayingPanelProps) => {
  const title = useStudioStore((state) => state.nowPlayingTitle);
  const artist = useStudioStore((state) => state.nowPlayingArtist);
  const setNowPlaying = useStudioStore((state) => state.setNowPlaying);
  const pushed = useStudioStore((state) => state.metadataPushed);
  const markPushed = useStudioStore((state) => state.markMetadataPushed);

  const handlePush = useCallback(() => {
    void onPush(title, artist).then(() => markPushed(true));
  }, [onPush, title, artist, markPushed]);

  return (
    <Panel title={<SectionLabel tracking="widest">NOW PLAYING</SectionLabel>} p="3xl">
      <TextInput
        value={title}
        placeholder="track title"
        onChange={(event) => {
          setNowPlaying('title', event.currentTarget.value);
          markPushed(false);
        }}
      />
      <TextInput
        value={artist}
        placeholder="artist"
        onChange={(event) => {
          setNowPlaying('artist', event.currentTarget.value);
          markPushed(false);
        }}
      />
      <ActionButton tone="ghost" fullWidth padding="10px 0" size="xs" onClick={handlePush}>
        {pushed ? 'PUSHED TO SERVER ✓' : 'PUSH TO SERVER'}
      </ActionButton>
    </Panel>
  );
};
