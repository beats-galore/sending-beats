import { Text } from '@mantine/core';

import { useAudioMetrics } from '../../../hooks';
import { useMixerStore } from '../../../stores';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

/** The always-visible engine readout: rate, buffer, resulting latency and load. */
export const EngineLine = () => {
  const config = useMixerStore((state) => state.config);
  const metrics = useAudioMetrics();

  if (!config) {
    return null;
  }

  const latencyMs = ((config.buffer_size / config.sample_rate) * 1000).toFixed(1);
  const cpu = metrics ? `${Math.round(metrics.cpu_usage)}% CPU` : '— CPU';

  return (
    <Text
      size="xs"
      c={color.textDim}
      truncate
      style={{ letterSpacing: layout.tracking.tight, flex: '0 1 auto', minWidth: 0 }}
    >
      {config.sample_rate / 1000} kHz · {config.buffer_size} · {latencyMs} ms · {cpu}
    </Text>
  );
};
