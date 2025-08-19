// Channel-specific VU meter component
import { memo } from 'react';
import { Text, Center } from '@mantine/core';
import { VUMeter } from '../ui';

type ChannelVUMeterProps = {
  peakLevel: number;
  rmsLevel: number;
};

export const ChannelVUMeter = memo<ChannelVUMeterProps>(({
  peakLevel,
  rmsLevel
}) => {
  return (
    <Center>
      <div style={{ textAlign: 'center' }}>
        <VUMeter
          peakLevel={peakLevel}
          rmsLevel={rmsLevel}
          height={120}
          width={16}
          vertical={true}
        />
        <Text size="xs" c="dimmed" mt="xs">
          {peakLevel.toFixed(3)}
        </Text>
      </div>
    </Center>
  );
}, (prev, next) => 
  prev.peakLevel === next.peakLevel && prev.rmsLevel === next.rmsLevel
);