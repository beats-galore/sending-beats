import { Group, Stack } from '@mantine/core';

import { color } from '../../../theme/tokens';
import { asGain } from '../format';
import type { PatchChannelChain } from '../hooks/use-patch-channel';
import { DragColumn } from '../primitives/DragColumn';
import { SectionLabel } from '../primitives/SectionLabel';

const EQ_MIN = -12;
const EQ_MAX = 12;

type ChannelEqualizerProps = {
  chain: PatchChannelChain;
};

/** Three-band tone control for one channel. */
export const ChannelEqualizer = ({ chain }: ChannelEqualizerProps) => {
  const bands = [
    { label: 'LOW', value: chain.eqLowGain, apply: (v: number) => chain.setEq({ lowGain: v }) },
    { label: 'MID', value: chain.eqMidGain, apply: (v: number) => chain.setEq({ midGain: v }) },
    { label: 'HIGH', value: chain.eqHighGain, apply: (v: number) => chain.setEq({ highGain: v }) },
  ];

  const readingTone = (value: number) =>
    value > 0 ? color.acc : value < 0 ? color.hotText : color.textDim;

  return (
    <Stack w={150} gap="sm" style={{ flex: 'none' }}>
      <SectionLabel>3-BAND EQ</SectionLabel>
      <Group h={104} gap="4xl" justify="center" align="stretch" wrap="nowrap">
        {bands.map((band) => (
          <DragColumn
            key={band.label}
            label={band.label}
            value={band.value}
            min={EQ_MIN}
            max={EQ_MAX}
            onChange={(value) => void band.apply(Math.round(value * 10) / 10)}
            display={asGain(band.value).replace('dB', '')}
            displayTone={readingTone(band.value)}
          />
        ))}
      </Group>
    </Stack>
  );
};
