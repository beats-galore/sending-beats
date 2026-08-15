import { Group, Stack, Text } from '@mantine/core';
import { useCallback } from 'react';

import { useChannelEffects } from '../../../hooks';
import { useMixerStore } from '../../../stores';
import { border, color } from '../../../theme/tokens';
import type { AudioChannel } from '../../../types';
import { asPan } from '../format';
import { ActionButton } from '../primitives/ActionButton';
import { DragBar } from '../primitives/DragBar';
import { Pill } from '../primitives/Pill';
import { SectionLabel } from '../primitives/SectionLabel';
import { ChannelCompressor } from './ChannelCompressor';
import { ChannelEqualizer } from './ChannelEqualizer';

type ChannelInspectorProps = {
  channel: AudioChannel;
  pan: number;
  onPanChange: (pan: number) => void;
  sourceName: string;
  port: number;
  /**
   * Whether the node is tall enough for the chain below the switch.
   *
   * A node shows as much as it has room for. Switching the effects on inside a
   * node that has only been opened as far as the inspector leaves the chain
   * waiting until the node is made taller, rather than growing the node out
   * from under the click that switched it on.
   */
  showChain: boolean;
};

/** The processing chain for a channel, revealed inside its node. */
export const ChannelInspector = ({
  channel,
  pan,
  onPanChange,
  sourceName,
  port,
  showChain,
}: ChannelInspectorProps) => {
  const updateChannel = useMixerStore((state) => state.updateChannel);
  const { setLimiterThreshold, toggleLimiter } = useChannelEffects(channel.id);
  const chain = channel.effects_enabled && showChain;

  const toggleEffects = useCallback(() => {
    void updateChannel(channel.id, { effects_enabled: !channel.effects_enabled });
  }, [updateChannel, channel.id, channel.effects_enabled]);

  return (
    <Stack
      gap="xl"
      pt="lg"
      mt="3xs"
      style={{ borderTop: border() }}
      onClick={(event) => event.stopPropagation()}
    >
      <Group gap="sm" wrap="nowrap">
        <ActionButton
          tone={channel.effects_enabled ? 'accent' : 'ghost'}
          padding="6px 12px"
          onClick={toggleEffects}
        >
          {channel.effects_enabled ? 'FX ON' : 'FX OFF'}
        </ActionButton>
        {chain && (
          <>
            <SectionLabel tracking="wide">PAN</SectionLabel>
            <DragBar
              value={pan}
              min={-1}
              max={1}
              onChange={onPanChange}
              knob={[10, 14]}
              centerMark
            />
            <Text size="2xs" c={color.textDim} w={32} ta="right">
              {asPan(pan)}
            </Text>
          </>
        )}
      </Group>

      {chain && (
        <>
          <Group gap="xl" align="flex-start" wrap="nowrap">
            <ChannelEqualizer channel={channel} />
            <ChannelCompressor channel={channel} />
          </Group>

          <Group gap="sm" wrap="nowrap" pt="md" style={{ borderTop: border() }}>
            <SectionLabel tracking="wider">LIMITER</SectionLabel>
            <Pill
              tone={channel.limiter_enabled ? 'accent' : 'muted'}
              onClick={() => void toggleLimiter()}
            >
              {channel.limiter_enabled ? 'ON' : 'OFF'}
            </Pill>
            <DragBar
              value={channel.limiter_threshold}
              min={-12}
              max={0}
              onChange={(value) => void setLimiterThreshold(Math.round(value * 10) / 10)}
            />
            <Text size="2xs" c={color.textDim} w={52} ta="right">
              {channel.limiter_threshold.toFixed(1)} dB
            </Text>
          </Group>
        </>
      )}

      <Text size="2xs" c={color.textFaintest}>
        {sourceName} · MASTER SUM port {port}
      </Text>
    </Stack>
  );
};
