import { Text } from '@mantine/core';

import { usePipelineLatency } from '../../../hooks/use-pipeline-latency';
import { useProcessMetrics } from '../../../hooks/use-process-metrics';
import { useMixerStore } from '../../../stores';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { formatBytes } from '../../../types/process-metrics.types';

/** The always-visible engine readout: rate, measured monitoring latency and load. */
export const EngineLine = () => {
  const config = useMixerStore((state) => state.config);
  const latency = usePipelineLatency();
  const process = useProcessMetrics();

  if (!config) {
    return null;
  }

  // Measured, not derived from a configured buffer size: nothing in the running
  // pipeline uses that number.
  const latencyMs =
    latency && latency.monitor_micros > 0 ? `${(latency.monitor_micros / 1000).toFixed(1)} ms` : '—';
  const cpu = process?.cpu_ready ? `${Math.round(process.cpu_percent)}% CPU` : '— CPU';
  const memory = process ? formatBytes(process.memory_bytes) : '—';

  return (
    <Text
      size="xs"
      c={color.textDim}
      truncate
      style={{ letterSpacing: layout.tracking.tight, flex: '0 1 auto', minWidth: 0 }}
    >
      {config.sample_rate / 1000} kHz · {latencyMs} · {cpu} · {memory}
    </Text>
  );
};
