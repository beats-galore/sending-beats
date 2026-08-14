import { Text, TextInput } from '@mantine/core';
import { useCallback, useEffect, useState } from 'react';

import { useMixerStore } from '../../../stores/mixer-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

type ChannelNameProps = {
  channelId: number;
  /** The stored name, empty when the channel has not been named */
  name: string;
  /** What the channel is patched to, shown in place of a name */
  deviceName: string | null;
};

/**
 * The channel's title, edited in place.
 *
 * Channels start unnamed rather than as "Deck A" or "Microphone", which describe
 * a rig the user may not have. Until one is given a name it borrows the name of
 * whatever is patched into it, so the strip still says something useful.
 */
export const ChannelName = ({ channelId, name, deviceName }: ChannelNameProps) => {
  const renameChannel = useMixerStore((state) => state.renameChannel);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(name);

  // A rename from elsewhere, or a config reload, should win over a stale draft
  useEffect(() => {
    if (!editing) {
      setDraft(name);
    }
  }, [name, editing]);

  const commit = useCallback(() => {
    setEditing(false);
    if (draft.trim() === name) {
      return;
    }
    void renameChannel(channelId, draft);
  }, [channelId, draft, name, renameChannel]);

  if (editing) {
    return (
      <TextInput
        value={draft}
        autoFocus
        onChange={(event) => setDraft(event.currentTarget.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            commit();
          }
          if (event.key === 'Escape') {
            setDraft(name);
            setEditing(false);
          }
        }}
        onClick={(event) => event.stopPropagation()}
        variant="unstyled"
        placeholder="name this channel"
        style={{ flex: 1, minWidth: 0 }}
        styles={{
          input: {
            fontFamily: 'var(--mantine-font-family-headings)',
            fontWeight: 600,
            fontSize: 'var(--mantine-font-size-lg)',
            letterSpacing: layout.tracking.tight,
            color: color.text,
          },
        }}
      />
    );
  }

  const fallback = deviceName ?? 'unnamed';

  return (
    <Text
      ff="var(--mantine-font-family-headings)"
      fw={600}
      fz="lg"
      truncate
      title="Click to rename"
      c={name ? undefined : color.textFaint}
      onClick={(event) => {
        event.stopPropagation();
        setEditing(true);
      }}
      style={{ flex: 1, letterSpacing: layout.tracking.tight, cursor: 'text' }}
    >
      {name || fallback}
    </Text>
  );
};
