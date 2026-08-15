import { Text } from '@mantine/core';

import { channelTargetKey } from '../../../services/patch-color-service';
import { useMixerStore } from '../../../stores/mixer-store';
import { useStudioStore } from '../../../stores/studio-store';
import { color } from '../../../theme/tokens';
import type { AudioChannel } from '../../../types';
import { useChannelNowPlaying } from '../hooks/use-channel-now-playing';
import { useChannelSource } from '../hooks/use-channel-source';
import { useNodeDrag, useNodeFront } from '../hooks/use-node-drag';
import { useNodeResize, useNodeSize, useUnshrink } from '../hooks/use-node-resize';
import { usePatchChannel } from '../hooks/use-patch-channel';
import { DeleteButton } from '../primitives/DeleteButton';
import { ExpandToggle } from '../primitives/ExpandToggle';
import { PortDot } from '../primitives/PortDot';
import { AppCard } from './AppCard';
import { ChannelBody } from './ChannelBody';
import { ChannelCompact } from './ChannelCompact';
import { ChannelName } from './ChannelName';
import { DeviceCard } from './DeviceCard';
import {
  channelPortOffset,
  channelSize,
  expansionFor,
  nextExpansion,
  openExpansion,
} from './patch-geometry';
import type { ChannelCardVariant, ChannelExpansion } from './patch-geometry';
import type { NodeRect } from './patch-layout';
import { PatchBadge } from './PatchBadge';

type ChannelNodeProps = {
  channel: AudioChannel;
  index: number;
  /** Box in canvas coordinates, with anything the user arranged applied. */
  rect: NodeRect;
  variant: ChannelCardVariant;
  /** Holds the ring and the keyboard shortcuts. Does not open the node. */
  selected: boolean;
};

/**
 * The sizes the toggle steps through, in order.
 *
 * A channel with its effects switched off has no chain to make room for, so the
 * rung above the ordinary card is the inspector rather than the whole thing.
 */
const toggleOrder = (effectsEnabled: boolean): ChannelExpansion[] => [
  'compact',
  'collapsed',
  openExpansion(effectsEnabled),
];

/** One source on the patch canvas: what it is, how loud, and how it is processed. */
export const ChannelNode = ({ channel, index, rect, variant, selected }: ChannelNodeProps) => {
  const select = useStudioStore((state) => state.select);
  const removeChannel = useMixerStore((state) => state.removeChannel);
  const patch = usePatchChannel(channel);
  const source = useChannelSource(channel.id);
  const track = useChannelNowPlaying(source.configuredDevice?.deviceIdentifier);

  const targetKey = channelTargetKey(channel.id);
  const grab = useNodeDrag(targetKey, rect);
  const resize = useNodeResize(targetKey, rect, channelSize(variant, 'compact'));
  const setSize = useNodeSize(targetKey);
  const { front, bringToFront } = useNodeFront(targetKey);

  // How much of the node is showing follows from how big it is, so the toggle
  // only has to size it to whatever it is going to show next.
  const expansion = expansionFor(variant, rect);
  const order = toggleOrder(channel.effects_enabled);
  const next = nextExpansion(expansion, order);
  const unshrink = useUnshrink(
    targetKey,
    expansion === 'compact',
    channelSize(variant, 'collapsed')
  );

  // An unavailable source outranks mute and tap styling: the channel is patched
  // to something that cannot deliver audio, which the user has to see.
  const unavailable = source.unavailableReason !== null;
  const tone = unavailable
    ? 'hot'
    : patch.muted
      ? 'dead'
      : source.isApplicationTap
        ? 'warn'
        : 'accent';

  const card = {
    rect,
    selected,
    raised: front,
    borderColor: selected ? color.acc : color.line,
    onPress: bringToFront,
    onClick: () => {
      select({ kind: 'channel', channelId: channel.id });
      unshrink();
    },
    onGrab: grab,
    onResize: resize,
    ports: <PortDot tone={tone} side="right" top={channelPortOffset} />,
    header: (
      <>
        <PatchBadge
          targetKey={targetKey}
          position={index}
          dimmed={unavailable || patch.muted}
          label="SOURCE COLOUR"
        />
        <ChannelName
          channelId={channel.id}
          name={channel.name}
          deviceName={source.configuredDevice?.deviceName ?? null}
          editable={selected}
        />
        <Text size="2xs" c={color.textFaint}>
          ⌥{index + 1}
        </Text>
        <ExpandToggle
          grows={order.indexOf(next) > order.indexOf(expansion)}
          onToggle={() => setSize(channelSize(variant, next))}
        />
        <DeleteButton
          onDelete={() => void removeChannel(channel.id)}
          title={`Remove ${channel.name} from the mix`}
        />
      </>
    ),
  };

  const body =
    expansion === 'compact' ? (
      <ChannelCompact patch={patch} />
    ) : (
      <ChannelBody
        channel={channel}
        index={index}
        expansion={expansion}
        patch={patch}
        source={source}
      />
    );

  if (variant === 'app') {
    return (
      <AppCard {...card} track={track}>
        {body}
      </AppCard>
    );
  }

  return <DeviceCard {...card}>{body}</DeviceCard>;
};
