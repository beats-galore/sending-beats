import { Box, Group, SimpleGrid, Stack, Text } from '@mantine/core';

import { useChannelEffects } from '../../../hooks';
import { border, color } from '../../../theme/tokens';
import type { AudioChannel } from '../../../types';
import { ParamTile } from '../primitives/ParamTile';
import { Pill } from '../primitives/Pill';
import { SectionLabel } from '../primitives/SectionLabel';


type ChannelCompressorProps = {
  channel: AudioChannel;
};

/** Dynamics for one channel: the four parameters and the engage switch. */
export const ChannelCompressor = ({ channel }: ChannelCompressorProps) => {
  const {
    setCompressorThreshold,
    setCompressorRatio,
    setCompressorAttack,
    setCompressorRelease,
    toggleCompressor,
  } = useChannelEffects(channel.id);

  const params = [
    {
      label: 'THRESHOLD',
      unit: 'dB',
      value: channel.comp_threshold,
      min: -40,
      max: 0,
      precision: 1,
      apply: setCompressorThreshold,
    },
    {
      label: 'RATIO',
      unit: ': 1',
      value: channel.comp_ratio,
      min: 1,
      max: 10,
      precision: 1,
      apply: setCompressorRatio,
    },
    {
      label: 'ATTACK',
      unit: 'ms',
      value: channel.comp_attack,
      min: 0.1,
      max: 100,
      precision: 0,
      apply: setCompressorAttack,
    },
    {
      label: 'RELEASE',
      unit: 'ms',
      value: channel.comp_release,
      min: 10,
      max: 1000,
      precision: 0,
      apply: setCompressorRelease,
    },
  ];

  return (
    <Stack gap="sm" style={{ flex: 1, minWidth: 0 }}>
      <Group gap="sm" wrap="nowrap">
        <SectionLabel>COMPRESSOR</SectionLabel>
        <Pill
          tone={channel.comp_enabled ? 'accent' : 'muted'}
          onClick={() => void toggleCompressor()}
        >
          {channel.comp_enabled ? 'ON' : 'OFF'}
        </Pill>
        <Box style={{ flex: 1, height: 1, background: color.line }} />
        <SectionLabel tracking="caps">GAIN REDUCTION</SectionLabel>
        {/* The engine does not report gain reduction yet, so the meter stays empty
            rather than showing a reading the processor is not actually producing. */}
        <Box
          w={54}
          h={5}
          style={{
            flex: 'none',
            background: color.bg,
            border: border(),
            borderRadius: 'var(--mantine-radius-xs)',
          }}
        />
        <Text size="3xs" c={color.textFaintest}>
          —
        </Text>
      </Group>

      <SimpleGrid cols={2} spacing="sm">
        {params.map((param) => (
          <ParamTile
            key={param.label}
            label={param.label}
            unit={param.unit}
            value={param.value}
            min={param.min}
            max={param.max}
            precision={param.precision}
            onChange={(value) => void param.apply(Math.round(value * 10) / 10)}
          />
        ))}
      </SimpleGrid>
    </Stack>
  );
};
