import { Group, SimpleGrid, Stack } from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { color } from '../../../theme/tokens';
import { asBytes, asElapsed } from '../format';
import { useCastTelemetry } from '../hooks/use-cast-telemetry';
import { useListenerStats } from '../hooks/use-listener-stats';
import { useStreamTransport } from '../hooks/use-stream-transport';
import { StatTile } from '../primitives/StatTile';
import { ConnectionLog } from './ConnectionLog';
import { ListenerSparkline } from './ListenerSparkline';
import { TransmitterPanel } from './TransmitterPanel';

/** Streaming: where the mix goes, and how the connection is holding up. */
export const CastView = () => {
  const stream = useStudioStore((state) => state.stream);
  const { isLive, isBusy, status, uptimeSeconds, toggle } = useStreamTransport();
  const listeners = useListenerStats(isLive);

  const { series, log } = useCastTelemetry({
    isLive,
    listeners: listeners.current,
    bitrate: stream.bitrate,
    variableBitrate: stream.variableBitrate,
    lastError: status?.last_error ?? null,
  });

  const sent = status?.icecast_stats?.bytes_sent;

  return (
    <Group
      align="stretch"
      gap="4xl"
      p="5xl"
      wrap="nowrap"
      style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}
    >
      {/* Metadata is not set here. What is playing comes from the mix, and the
          on-air drawer is where it is corrected while broadcasting. */}
      <Stack w={420} gap="2xl" style={{ flex: 'none' }}>
        <TransmitterPanel isLive={isLive} isBusy={isBusy} onToggle={() => void toggle()} />
      </Stack>

      <Stack gap="2xl" style={{ flex: 1, minWidth: 0 }}>
        <SimpleGrid cols={4} spacing="xl">
          <StatTile label="LISTENERS" value={listeners.current ?? '—'} tone={color.acc} />
          <StatTile label="UPTIME" value={isLive ? asElapsed(uptimeSeconds) : '—'} />
          <StatTile label="SENT" value={sent ? asBytes(sent) : '—'} />
          <StatTile label="PEAK" value={listeners.peak ?? '—'} />
        </SimpleGrid>

        <ListenerSparkline series={series} />
        <ConnectionLog entries={log} />
      </Stack>
    </Group>
  );
};
