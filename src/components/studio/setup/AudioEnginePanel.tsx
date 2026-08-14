import { Group, SimpleGrid, Text } from '@mantine/core';

import { usePipelineLatency } from '../../../hooks/use-pipeline-latency';
import { useProcessMetrics } from '../../../hooks/use-process-metrics';
import { useMixerStore } from '../../../stores';
import { border, color } from '../../../theme/tokens';
import { formatBytes } from '../../../types/process-metrics.types';
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
  const latency = usePipelineLatency();
  const process = useProcessMetrics();

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
            {process?.cpu_ready ? `${Math.round(process.cpu_percent)}%` : '—'}
          </Text>
        </Text>
        <Text size="xs" c={color.textDim}>
          MEMORY{' '}
          <Text span c={color.text}>
            {process ? formatBytes(process.memory_bytes) : '—'}
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
