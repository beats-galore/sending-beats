import { Group, Text } from '@mantine/core';
import { motion } from 'framer-motion';

import { layout } from '../../../theme/layout';
import { border, color, glow } from '../../../theme/tokens';
import { asClock } from '../format';
import { useStreamTransport } from '../hooks/use-stream-transport';

/** On-air state and uptime, and the fastest way to start or cut the stream. */
export const LiveChip = () => {
  const { isLive, isBusy, uptimeSeconds, toggle } = useStreamTransport();

  return (
    <Group
      onClick={() => void toggle()}
      gap="sm"
      wrap="nowrap"
      style={{
        flex: 'none',
        padding: '6px 12px',
        borderRadius: 'var(--mantine-radius-sm)',
        cursor: isBusy ? 'wait' : 'pointer',
        border: border(isLive ? 'hotBorder' : 'line'),
        background: isLive ? color.hotBg : undefined,
        color: isLive ? color.hotText : color.textDim,
      }}
    >
      <motion.div
        animate={isLive ? { opacity: [1, 0.35, 1] } : { opacity: 1 }}
        transition={isLive ? { duration: 1.5, repeat: Infinity, ease: 'easeInOut' } : undefined}
        style={{
          width: 7,
          height: 7,
          borderRadius: '50%',
          background: isLive ? color.hot : color.textFaintest,
          boxShadow: isLive ? glow('hot') : undefined,
        }}
      />
      <Text size="xs" fw={600} c="inherit" style={{ letterSpacing: layout.tracking.heading }}>
        {isLive ? `LIVE ${asClock(uptimeSeconds)}` : 'OFF AIR'}
      </Text>
    </Group>
  );
};
