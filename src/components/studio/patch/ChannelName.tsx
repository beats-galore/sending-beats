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
  /**
   * Renaming is offered only on the focused node. On any other node the title
   * is just the title — though the first click of a double click focuses the
   * node, so reaching for a rename gets there in the same gesture.
   */
  editable: boolean;
};

/**
 * The channel's title, edited in place.
 *
 * Channels start unnamed rather than as "Deck A" or "Microphone", which describe
 * a rig the user may not have. Until one is given a name it borrows the name of
 * whatever is patched into it, so the strip still says something useful.
 *
 * Opened on a double click rather than a single one. The title is the widest
 * part of the title bar and the title bar is the grip that moves the node, so a
 * single click has to stay a press that might become a drag — anything else
 * turns every attempt to move a card into a rename.
 */
export const ChannelName = ({ channelId, name, deviceName, editable }: ChannelNameProps) => {
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

  // A node that loses focus while being renamed keeps the edit rather than
  // stranding an open field on a collapsed card.
  useEffect(() => {
    if (!editable && editing) {
      commit();
    }
  }, [editable, editing, commit]);

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
      title="Double click to edit"
      c={name ? undefined : color.textFaint}
      // Gated on the node being focused, which the first click of the double
      // click sees to — and which the effect above relies on, since it closes
      // an edit the moment the node it belongs to stops being focused.
      onDoubleClick={
        editable
          ? (event) => {
              event.stopPropagation();
              setEditing(true);
            }
          : undefined
      }
      style={{
        flex: 1,
        letterSpacing: layout.tracking.tight,
        // Inherited from the title bar, which is the grip: a single press here
        // is the start of a drag, and a text caret would promise otherwise.
        cursor: 'inherit',
      }}
    >
      {name || fallback}
    </Text>
  );
};
