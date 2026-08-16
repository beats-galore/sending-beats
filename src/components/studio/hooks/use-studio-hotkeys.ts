import { useHotkeys } from '@mantine/hooks';

import { useChannelsData } from '../../../hooks';
import { useStudioStore } from '../../../stores/studio-store';
import { DEFAULT_CHANNEL } from '../../../types';
import type { AudioChannel } from '../../../types';
import { useFocusedChannelId } from './use-focused-node';
import { usePatchChannel } from './use-patch-channel';
import { useStreamTransport } from './use-stream-transport';
import { useTapeTransport } from './use-tape-transport';

// Stands in when nothing is patched, so the hook order stays stable. Its setters
// no-op because it resolves to no configured device.
const NO_CHANNEL: AudioChannel = { ...DEFAULT_CHANNEL, id: -1, name: '' };

const FOCUS_KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];

/** Transport and channel shortcuts, as listed in SETUP. */
export const useStudioHotkeys = () => {
  const { channels } = useChannelsData();
  const select = useStudioStore((state) => state.select);

  // Mute and solo act on the focused channel, so they no-op while a destination
  // holds the focus rather than reaching for a channel that is not on screen.
  const focusedChannelId = useFocusedChannelId();
  const selected = channels.find((channel) => channel.id === focusedChannelId) ?? NO_CHANNEL;

  const patch = usePatchChannel(selected);
  const stream = useStreamTransport();
  const tape = useTapeTransport();

  useHotkeys([
    ['m', () => patch.setMuted()],
    ['s', () => patch.setSolo()],
    ['mod+R', () => void tape.toggle()],
    ['mod+L', () => void stream.toggle()],
    ...FOCUS_KEYS.map((key, index): [string, () => void] => [
      `alt+${key}`,
      () => {
        if (index < channels.length) {
          select({ kind: 'channel', channelId: channels[index].id });
        }
      },
    ]),
  ]);
};
