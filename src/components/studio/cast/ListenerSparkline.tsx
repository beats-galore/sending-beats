import { Box, Group, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import { Panel } from '../primitives/Panel';
import { SectionLabel } from '../primitives/SectionLabel';

type ListenerSparklineProps = {
  series: number[];
};

/** Listener count over the session so far. Empty until the first poll lands. */
export const ListenerSparkline = ({ series }: ListenerSparklineProps) => {
  const peak = Math.max(1, ...series);

  return (
    <Panel title={<SectionLabel tracking="widest">LISTENERS, THIS SESSION</SectionLabel>} p="3xl">
      <Group h={120} align="flex-end" gap="2xs" wrap="nowrap">
        {series.length === 0 ? (
          <Text size="xs" c={color.textFaint} m="auto">
            No listener data yet.
          </Text>
        ) : (
          series.map((value, index) => (
            <Box
              key={index}
              style={{
                flex: 1,
                height: `${Math.max(4, (value / peak) * 100)}%`,
                borderRadius: 'var(--mantine-radius-2xs)',
                background: index === series.length - 1 ? color.acc : color.accDim,
              }}
            />
          ))
        )}
      </Group>

      <Group justify="space-between">
        <Text size="2xs" c={color.textFaintest}>
          session start
        </Text>
        <Text size="2xs" c={color.textFaintest}>
          now
        </Text>
      </Group>
    </Panel>
  );
};
