import { Group, Stack } from '@mantine/core';

import { useChannelEffects } from '../../../hooks';
import { color } from '../../../theme/tokens';
import type { AudioChannel } from '../../../types';
import { asGain } from '../format';
import { DragColumn } from '../primitives/DragColumn';
import { SectionLabel } from '../primitives/SectionLabel';

const EQ_MIN = -12;
const EQ_MAX = 12;

type ChannelEqualizerProps = {
  channel: AudioChannel;
};

/** Three-band tone control for one channel. */
export const ChannelEqualizer = ({ channel }: ChannelEqualizerProps) => {
  const { setEQLowGain, setEQMidGain, setEQHighGain } = useChannelEffects(channel.id);

  const bands = [
    { label: 'LOW', value: channel.eq_low_gain, apply: setEQLowGain },
    { label: 'MID', value: channel.eq_mid_gain, apply: setEQMidGain },
    { label: 'HIGH', value: channel.eq_high_gain, apply: setEQHighGain },
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
