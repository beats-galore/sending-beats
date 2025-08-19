// Channel EQ controls (3-band equalizer)
import { Stack, Button } from '@mantine/core';
import { memo } from 'react';

import { useChannelEffects } from '../../hooks';
import { AudioSlider } from '../ui';

type ChannelEQProps = {
  channelId: number;
};

export const ChannelEQ = memo<ChannelEQProps>(({ channelId }) => {
  const { setEQLowGain, setEQMidGain, setEQHighGain, resetEQ, getEQValues } =
    useChannelEffects(channelId);

  const eqValues = getEQValues();

  if (!eqValues) return null;

  return (
    <Stack gap="sm">
      {/* High Frequencies */}
      <AudioSlider
        label="High"
        value={eqValues.high.value}
        min={-12}
        max={12}
        step={0.5}
        unit="dB"
        onChange={setEQHighGain}
      />

      {/* Mid Frequencies */}
      <AudioSlider
        label="Mid"
        value={eqValues.mid.value}
        min={-12}
        max={12}
        step={0.5}
        unit="dB"
        onChange={setEQMidGain}
      />

      {/* Low Frequencies */}
      <AudioSlider
        label="Low"
        value={eqValues.low.value}
        min={-12}
        max={12}
        step={0.5}
        unit="dB"
        onChange={setEQLowGain}
      />

      {/* Reset Button */}
      <Button size="xs" variant="subtle" onClick={resetEQ} color="gray">
        Reset EQ
      </Button>
    </Stack>
  );
});
