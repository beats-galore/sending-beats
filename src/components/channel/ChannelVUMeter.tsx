// Channel-specific VU meter component
import { Text, Center } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

import { VUMeter } from '../ui';

const useStyles = createStyles((theme) => ({
  textCenter: {
    textAlign: 'center',
  },
}));

type ChannelVUMeterProps = {
  peakLevel: number;
  rmsLevel: number;
};

export const ChannelVUMeter = memo<ChannelVUMeterProps>(
  ({ peakLevel, rmsLevel }) => {
    const { classes } = useStyles();
    
    return (
      <Center>
        <div className={classes.textCenter}>
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
  },
  (prev, next) => prev.peakLevel === next.peakLevel && prev.rmsLevel === next.rmsLevel
);
