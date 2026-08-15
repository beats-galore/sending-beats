import { Group, Stack, Text } from '@mantine/core';

import { useChannelEffects } from '../../../hooks';
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
   * A node shows as much as it has room for, and the chain needs a good deal
   * more room than the switch that turns it on. Switching it on is what asks
   * for that room — see `onToggleEffects` — so this is normally true by the
   * time the chain exists to be drawn.
   */
  showChain: boolean;
  /** Switches the chain on or off, and makes room for it when switching it on. */
  onToggleEffects: () => void;
};

/** The processing chain for a channel, revealed inside its node. */
export const ChannelInspector = ({
  channel,
  pan,
  onPanChange,
  sourceName,
  port,
  showChain,
  onToggleEffects,
}: ChannelInspectorProps) => {
  const { setLimiterThreshold, toggleLimiter } = useChannelEffects(channel.id);
  const chain = channel.effects_enabled && showChain;

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
          onClick={onToggleEffects}
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
