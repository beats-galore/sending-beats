import { Box, Group, NativeSelect, Stack, Text } from '@mantine/core';

import { channelTargetKey } from '../../../services/patch-color-service';
import { useMixerStore } from '../../../stores/mixer-store';
import { useStudioStore } from '../../../stores/studio-store';
import { color } from '../../../theme/tokens';
import type { AudioChannel } from '../../../types';
import { asGain, meterPosition } from '../format';
import { useChannelNowPlaying } from '../hooks/use-channel-now-playing';
import { useChannelSource } from '../hooks/use-channel-source';
import { useNodeDrag, useNodeFront } from '../hooks/use-node-drag';
import { useNodeResize, useNodeSize } from '../hooks/use-node-resize';
import { usePatchChannel } from '../hooks/use-patch-channel';
import { DeleteButton } from '../primitives/DeleteButton';
import { DragBar } from '../primitives/DragBar';
import { ExpandToggle } from '../primitives/ExpandToggle';
import { LevelMeter } from '../primitives/LevelMeter';
import { Pill } from '../primitives/Pill';
import { PortDot } from '../primitives/PortDot';
import { StatusDot } from '../primitives/StatusDot';
import { AppCard } from './AppCard';
import { ChannelInspector } from './ChannelInspector';
import { ChannelName } from './ChannelName';
import { DestinationTiles } from './DestinationTiles';
import { DeviceCard } from './DeviceCard';
import { channelPortOffset, channelSize, expansionFor, openExpansion } from './patch-geometry';
import type { ChannelCardVariant } from './patch-geometry';
import type { NodeRect } from './patch-layout';
import { PatchBadge } from './PatchBadge';

const GAIN_MIN = -60;
const GAIN_MAX = 12;

type ChannelNodeProps = {
  channel: AudioChannel;
  index: number;
  /** Box in canvas coordinates, with anything the user arranged applied. */
  rect: NodeRect;
  variant: ChannelCardVariant;
  /** Holds the ring and the keyboard shortcuts. Does not open the node. */
  selected: boolean;
};

/** One source on the patch canvas: what it is, how loud, and how it is processed. */
export const ChannelNode = ({ channel, index, rect, variant, selected }: ChannelNodeProps) => {
  const select = useStudioStore((state) => state.select);
  const removeChannel = useMixerStore((state) => state.removeChannel);
  const patch = usePatchChannel(channel);
  const source = useChannelSource(channel.id);
  const track = useChannelNowPlaying(source.configuredDevice?.deviceIdentifier);

  const targetKey = channelTargetKey(channel.id);
  const shut = channelSize(variant, 'collapsed');
  const grab = useNodeDrag(targetKey, rect);
  const resize = useNodeResize(targetKey, rect, shut);
  const setSize = useNodeSize(targetKey);
  const { front, bringToFront } = useNodeFront(targetKey);

  // How much of the node is showing follows from how big it is, so the toggle
  // only has to size it to whatever it is going to show.
  const expansion = expansionFor(variant, rect);
  const expanded = expansion !== 'collapsed';
  const toggle = () =>
    setSize(
      expanded ? shut : channelSize(variant, openExpansion(channel.effects_enabled))
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
    onClick: () => select({ kind: 'channel', channelId: channel.id }),
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
        <ExpandToggle expanded={expanded} onToggle={toggle} />
        <DeleteButton
          onDelete={() => void removeChannel(channel.id)}
          title={`Remove ${channel.name} from the mix`}
        />
      </>
    ),
  };

  const body = (
    <>
      <Group gap="xs" wrap="nowrap">
        <StatusDot
          tone={tone === 'dead' ? 'inert' : tone}
          title={source.unavailableReason ?? undefined}
        />
        <NativeSelect
          value={source.configuredDevice?.deviceIdentifier ?? ''}
          onChange={(event) => void source.setSource(event.currentTarget.value)}
          onClick={(event) => event.stopPropagation()}
          data={[{ value: '', label: 'No input' }, ...source.options]}
          variant="unstyled"
          style={{ flex: 1, minWidth: 0 }}
          styles={{
            input: {
              color: unavailable ? color.hotText : color.textDim,
              fontSize: 'var(--mantine-font-size-xs)',
            },
          }}
        />
        {unavailable ? (
          <Pill tone="hot" size="3xs" title={source.unavailableReason ?? undefined}>
            OFFLINE
          </Pill>
        ) : (
          <Text size="3xs" c={color.textFaintest} style={{ flex: 'none' }}>
            {patch.isMono ? 'MONO' : 'STEREO'}
          </Text>
        )}
      </Group>

      <Stack gap="3xs">
        <LevelMeter level={meterPosition(patch.levels.left.peak)} dimmed={patch.muted} />
        <LevelMeter level={meterPosition(patch.levels.right.peak)} dimmed={patch.muted} />
      </Stack>

      <Group gap="sm" wrap="nowrap">
        <Text size="3xs" c={color.textFaint} w={12} style={{ flex: 'none' }}>
          {patch.isMono ? 'M' : 'LR'}
        </Text>
        <DragBar
          value={patch.gainDb}
          min={GAIN_MIN}
          max={GAIN_MAX}
          onChange={patch.setGain}
          tone={patch.muted ? 'muted' : 'accent'}
          knob={[10, 14]}
        />
        <Text size="2xs" w={46} ta="right" c={patch.muted ? color.textDim : color.acc}>
          {asGain(patch.gainDb)}
        </Text>
      </Group>

      <Group gap="xs" wrap="nowrap">
        {/* Sits in the space mute and solo already left, so showing where a
            source goes costs the card no extra height. */}
        <Box style={{ flex: 1, minWidth: 0 }}>
          <DestinationTiles
            deviceIdentifier={source.configuredDevice?.deviceIdentifier ?? null}
          />
        </Box>
        <Pill
          tone={patch.muted ? 'hot' : 'muted'}
          filled={patch.muted}
          onClick={(event) => {
            event.stopPropagation();
            patch.setMuted();
          }}
        >
          M
        </Pill>
        <Pill
          tone={patch.solo ? 'warn' : 'muted'}
          filled={patch.solo}
          onClick={(event) => {
            event.stopPropagation();
            patch.setSolo();
          }}
        >
          S
        </Pill>
      </Group>

      {expanded && (
        <ChannelInspector
          channel={channel}
          pan={patch.pan}
          onPanChange={patch.setPan}
          sourceName={patch.sourceName}
          port={index + 1}
          showChain={expansion === 'effects'}
        />
      )}
    </>
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
