import { useMemo } from 'react';

import { useChannelsData } from '../../../hooks';
import { useStudioStore } from '../../../stores/studio-store';
import type { StudioSelection } from '../../../stores/studio-store';

/**
 * The focused node, with the opening default applied.
 *
 * Nothing selected focuses the first channel, so the canvas never opens with
 * every node shut. Once anything has been focused the selection stands on its
 * own — focusing a destination has to be able to close the channel, which a
 * plain fallback to the first channel would undo.
 */
export const useFocusedNode = (): StudioSelection | null => {
  const { channels } = useChannelsData();
  const selection = useStudioStore((state) => state.selection);
  const firstChannelId = channels.length > 0 ? channels[0].id : null;

  return useMemo(() => {
    if (selection !== null) {
      return selection;
    }
    return firstChannelId === null ? null : { kind: 'channel', channelId: firstChannelId };
  }, [selection, firstChannelId]);
};

/** The focused channel, or null when a destination holds the focus instead. */
export const useFocusedChannelId = (): number | null => {
  const focused = useFocusedNode();
  return focused?.kind === 'channel' ? focused.channelId : null;
};
