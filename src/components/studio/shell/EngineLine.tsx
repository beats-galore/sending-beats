import { Text } from '@mantine/core';

import { useAudioMetrics } from '../../../hooks/use-audio-metrics';
import { usePipelineLatency } from '../../../hooks/use-pipeline-latency';
import { useMixerStore } from '../../../stores';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

/** The always-visible engine readout: rate, measured monitoring latency and load. */
export const EngineLine = () => {
  const config = useMixerStore((state) => state.config);
  const metrics = useAudioMetrics();
  const latency = usePipelineLatency();

  if (!config) {
    return null;
  }

  // Measured, not derived from a configured buffer size: nothing in the running
  // pipeline uses that number, and it understated the real figure several times over.
  const latencyMs =
    latency && latency.monitor_micros > 0 ? `${(latency.monitor_micros / 1000).toFixed(1)} ms` : '—';
  const cpu = metrics ? `${Math.round(metrics.cpu_usage)}% CPU` : '— CPU';

  return (
    <Text
      size="xs"
      c={color.textDim}
      truncate
      style={{ letterSpacing: layout.tracking.tight, flex: '0 1 auto', minWidth: 0 }}
    >
      {config.sample_rate / 1000} kHz · {latencyMs} · {cpu}
    </Text>
  );
};
