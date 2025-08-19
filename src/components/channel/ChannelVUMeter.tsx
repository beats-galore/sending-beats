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
        <div style={{ width: '100%', minWidth: '200px', display: 'flex', justifyContent: 'center' }}>
          <div style={{ flexGrow: 1, minWidth: '200px', maxWidth: '400px' }}>
            <VUMeter
              peakLevel={peakLevel}
              rmsLevel={rmsLevel}
              height={16}
              width={300}
              vertical={false}
            />
          </div>
        </div>
      </Center>
    );
  },
  (prev, next) => prev.peakLevel === next.peakLevel && prev.rmsLevel === next.rmsLevel
);
