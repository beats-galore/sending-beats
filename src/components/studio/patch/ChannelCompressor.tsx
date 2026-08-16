import { Box, Group, SimpleGrid, Stack, Text } from '@mantine/core';

import { border, color } from '../../../theme/tokens';
import type { PatchChannelChain } from '../hooks/use-patch-channel';
import { ParamTile } from '../primitives/ParamTile';
import { Pill } from '../primitives/Pill';
import { SectionLabel } from '../primitives/SectionLabel';

type ChannelCompressorProps = {
  chain: PatchChannelChain;
};

/** Dynamics for one channel: the four parameters and the engage switch. */
export const ChannelCompressor = ({ chain }: ChannelCompressorProps) => {
  const params = [
    {
      label: 'THRESHOLD',
      unit: 'dB',
      value: chain.compThreshold,
      min: -40,
      max: 0,
      precision: 1,
      apply: (value: number) => chain.setCompressor({ threshold: value }),
    },
    {
      label: 'RATIO',
      unit: ': 1',
      value: chain.compRatio,
      min: 1,
      max: 10,
      precision: 1,
      apply: (value: number) => chain.setCompressor({ ratio: value }),
    },
    {
      label: 'ATTACK',
      unit: 'ms',
      value: chain.compAttack,
      min: 0.1,
      max: 100,
      precision: 0,
      apply: (value: number) => chain.setCompressor({ attack: value }),
    },
    {
      label: 'RELEASE',
      unit: 'ms',
      value: chain.compRelease,
      min: 10,
      max: 1000,
      precision: 0,
      apply: (value: number) => chain.setCompressor({ release: value }),
    },
  ];

  return (
    <Stack gap="sm" style={{ flex: 1, minWidth: 0 }}>
      <Group gap="sm" wrap="nowrap">
        <SectionLabel>COMPRESSOR</SectionLabel>
        <Pill
          tone={chain.compEnabled ? 'accent' : 'muted'}
          onClick={() => chain.setCompressor({ enabled: !chain.compEnabled })}
        >
          {chain.compEnabled ? 'ON' : 'OFF'}
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
