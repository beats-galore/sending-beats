// Channel effects controls (compressor and limiter)
import { Stack, Button, Accordion, Switch, Group } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

import { useChannelEffects } from '../../hooks';
import { AudioSlider } from '../ui';

const useStyles = createStyles((theme) => ({
  fullWidth: {
    width: '100%',
  },
}));

type ChannelEffectsProps = {
  channelId: number;
};

export const ChannelEffects = memo<ChannelEffectsProps>(({ channelId }) => {
  const { classes } = useStyles();

  const {
    // Compressor
    setCompressorThreshold,
    setCompressorRatio,
    setCompressorAttack,
    setCompressorRelease,
    toggleCompressor,
    resetCompressor,
    getCompressorValues,

    // Limiter
    setLimiterThreshold,
    toggleLimiter,
    resetLimiter,
    getLimiterValues,

    // Combined
    resetAllEffects,
  } = useChannelEffects(channelId);

  const compressor = getCompressorValues();
  const limiter = getLimiterValues();

  if (!compressor || !limiter) return null;

  return (
    <Stack gap="sm">
      <Accordion variant="contained">
        {/* Compressor */}
        <Accordion.Item value="compressor">
          <Accordion.Control>
            <Group justify="space-between" className={classes.fullWidth}>
              <span>Compressor</span>
              <Switch
                checked={compressor.enabled}
                onChange={toggleCompressor}
                size="xs"
                onClick={(e) => e.stopPropagation()}
              />
            </Group>
          </Accordion.Control>
          <Accordion.Panel>
            <Stack gap="xs">
              <AudioSlider
                label="Threshold"
                value={compressor.threshold.value}
                min={-40}
                max={0}
                step={1}
                unit="dB"
                onChange={setCompressorThreshold}
                disabled={!compressor.enabled}
              />

              <AudioSlider
                label="Ratio"
                value={compressor.ratio.value}
                min={1}
                max={10}
                step={0.5}
                unit=":1"
                onChange={setCompressorRatio}
                disabled={!compressor.enabled}
              />

              <AudioSlider
                label="Attack"
                value={compressor.attack.value}
                min={0.1}
                max={100}
                step={0.5}
                unit="ms"
                onChange={setCompressorAttack}
                disabled={!compressor.enabled}
              />

              <AudioSlider
                label="Release"
                value={compressor.release.value}
                min={10}
                max={1000}
                step={10}
                unit="ms"
                onChange={setCompressorRelease}
                disabled={!compressor.enabled}
              />

              <Button size="xs" variant="subtle" onClick={resetCompressor} color="gray">
                Reset
              </Button>
            </Stack>
          </Accordion.Panel>
        </Accordion.Item>

        {/* Limiter */}
        <Accordion.Item value="limiter">
          <Accordion.Control>
            <Group justify="space-between" className={classes.fullWidth}>
              <span>Limiter</span>
              <Switch
                checked={limiter.enabled}
                onChange={toggleLimiter}
                size="xs"
                onClick={(e) => e.stopPropagation()}
              />
            </Group>
          </Accordion.Control>
          <Accordion.Panel>
            <Stack gap="xs">
              <AudioSlider
                label="Threshold"
                value={limiter.threshold.value}
                min={-12}
                max={0}
                step={0.5}
                unit="dB"
                onChange={setLimiterThreshold}
                disabled={!limiter.enabled}
              />

              <Button size="xs" variant="subtle" onClick={resetLimiter} color="gray">
                Reset
              </Button>
            </Stack>
          </Accordion.Panel>
        </Accordion.Item>
      </Accordion>

      {/* Reset All Effects */}
      <Button size="xs" variant="outline" onClick={resetAllEffects} color="red">
        Reset All Effects
      </Button>
    </Stack>
  );
});
