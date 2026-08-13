import { Box, Group, NativeSelect, Stack, Text } from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import type { AudioChannel } from '../../../types';
import { asGain, meterPosition } from '../format';
import { useChannelSource } from '../hooks/use-channel-source';
import { usePatchChannel } from '../hooks/use-patch-channel';
import { DragBar } from '../primitives/DragBar';
import { LevelMeter } from '../primitives/LevelMeter';
import { NodeCard } from '../primitives/NodeCard';
import { Pill } from '../primitives/Pill';
import { PortDot } from '../primitives/PortDot';
import { StatusDot } from '../primitives/StatusDot';
import { ChannelInspector } from './ChannelInspector';
import { channelHeight, channelWidth } from './patch-geometry';

const GAIN_MIN = -60;
const GAIN_MAX = 12;

type ChannelNodeProps = {
  channel: AudioChannel;
  index: number;
  top: number;
  expanded: boolean;
};

/** One source on the patch canvas: what it is, how loud, and how it is processed. */
export const ChannelNode = ({ channel, index, top, expanded }: ChannelNodeProps) => {
  const selectChannel = useStudioStore((state) => state.selectChannel);
  const patch = usePatchChannel(channel);
  const source = useChannelSource(channel.id);

  const tone = patch.muted ? 'dead' : source.isApplicationTap ? 'warn' : 'accent';

  return (
    <NodeCard
      position={{
        left: layout.source.x,
        top,
        width: channelWidth(expanded),
        height: channelHeight(expanded),
      }}
      selected={expanded}
      borderColor={expanded ? color.acc : color.line}
      onClick={() => selectChannel(channel.id)}
      ports={<PortDot tone={tone} side="right" top={layout.source.portOffset} />}
      header={
        <>
          <Pill tone={patch.muted ? 'muted' : source.isApplicationTap ? 'warn' : 'accent'} filled>
            {String(index + 1).padStart(2, '0')}
          </Pill>
          <Text
            ff="var(--mantine-font-family-headings)"
            fw={600}
            fz="lg"
            truncate
            style={{ flex: 1, letterSpacing: layout.tracking.tight }}
          >
            {channel.name}
          </Text>
          <Text size="2xs" c={color.textFaint}>
            ⌥{index + 1}
          </Text>
        </>
      }
      bodyStyle={{ display: 'flex', flexDirection: 'column', gap: 8, overflow: 'hidden' }}
    >
      <Group gap="xs" wrap="nowrap">
        <StatusDot tone={tone === 'dead' ? 'inert' : tone} />
        <NativeSelect
          value={source.configuredDevice?.deviceIdentifier ?? ''}
          onChange={(event) => void source.setSource(event.currentTarget.value)}
          onClick={(event) => event.stopPropagation()}
          data={[{ value: '', label: 'No input' }, ...source.options]}
          variant="unstyled"
          style={{ flex: 1, minWidth: 0 }}
          styles={{ input: { color: color.textDim, fontSize: 'var(--mantine-font-size-xs)' } }}
        />
        <Text size="3xs" c={color.textFaintest} style={{ flex: 'none' }}>
          {patch.isMono ? 'MONO' : 'STEREO'}
        </Text>
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
        <Box style={{ flex: 1 }} />
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
        />
      )}
    </NodeCard>
  );
};
