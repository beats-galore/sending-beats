import { useHotkeys } from '@mantine/hooks';

import { useChannelsData } from '../../../hooks';
import { useMixerStore } from '../../../stores';
import { useStudioStore } from '../../../stores/studio-store';
import { DEFAULT_CHANNEL } from '../../../types';
import type { AudioChannel } from '../../../types';
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
  const addChannel = useMixerStore((state) => state.addChannel);
  const selectedChannelId = useStudioStore((state) => state.selectedChannelId);
  const selectChannel = useStudioStore((state) => state.selectChannel);

  const selectedId = selectedChannelId ?? (channels.length > 0 ? channels[0].id : null);
  const selected = channels.find((channel) => channel.id === selectedId) ?? NO_CHANNEL;

  const patch = usePatchChannel(selected);
  const stream = useStreamTransport();
  const tape = useTapeTransport();

  useHotkeys([
    ['m', () => patch.setMuted()],
    ['s', () => patch.setSolo()],
    ['mod+R', () => void tape.toggle()],
    ['mod+L', () => void stream.toggle()],
    ['mod+N', () => void addChannel()],
    ...FOCUS_KEYS.map((key, index): [string, () => void] => [
      `alt+${key}`,
      () => {
        if (index < channels.length) {
          selectChannel(channels[index].id);
        }
      },
    ]),
  ]);
};
