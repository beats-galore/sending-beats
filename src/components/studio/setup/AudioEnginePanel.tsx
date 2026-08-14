import { Group, SimpleGrid, Text } from '@mantine/core';

import { useAudioMetrics } from '../../../hooks/use-audio-metrics';
import { usePipelineLatency } from '../../../hooks/use-pipeline-latency';
import { useMixerStore } from '../../../stores';
import { border, color } from '../../../theme/tokens';
import { Panel } from '../primitives/Panel';
import { StatTile } from '../primitives/StatTile';

/**
 * What the engine is running at.
 *
 * Sample rate is reported rather than editable — the engine negotiates it with
 * the hardware and exposes no command to change it. Latency is measured from
 * what every stage of the pipeline is currently holding, not derived from a
 * configured buffer size, so it reflects what a monitored microphone hears.
 */
export const AudioEnginePanel = () => {
  const config = useMixerStore((state) => state.config);
  const state = useMixerStore((store) => store.state);
  const metrics = useAudioMetrics();
  const latency = usePipelineLatency();

  const latencyMs =
    latency && latency.monitor_micros > 0 ? (latency.monitor_micros / 1000).toFixed(1) : '—';

  return (
    <Panel title="AUDIO ENGINE" p="3xl" gap="2xl">
      <SimpleGrid cols={2} spacing="xl">
        <StatTile
          label="SAMPLE RATE"
          value={config ? config.sample_rate / 1000 : '—'}
          unit="kHz"
          size="3xl"
        />
        <StatTile label="LATENCY" value={latencyMs} unit="ms" size="3xl" />
      </SimpleGrid>

      <Group gap="5xl" pt="xl" wrap="nowrap" style={{ borderTop: border() }}>
        <Text size="xs" c={color.textDim}>
          CPU{' '}
          <Text span c={color.text}>
            {metrics ? `${Math.round(metrics.cpu_usage)}%` : '—'}
          </Text>
        </Text>
        <Text size="xs" c={color.textDim}>
          UNDERRUNS{' '}
          <Text span c={color.text}>
            {metrics?.buffer_underruns ?? '—'}
          </Text>
        </Text>
        <Text size="xs" c={color.textDim}>
          ENGINE{' '}
          <Text span c={color.acc}>
            {state.toUpperCase()}
          </Text>
        </Text>
      </Group>
    </Panel>
  );
};
