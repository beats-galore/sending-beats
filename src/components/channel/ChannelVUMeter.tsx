// Channel-specific stereo VU meter component with separate L/R channels
import { Text, Center, Box, Stack } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

import { VUMeter } from '../ui';

const useStyles = createStyles((theme) => ({
  vuContainer: {
    width: '100%',
    minWidth: '200px',
    maxWidth: '300px',
    marginTop: 30,
  },
  channelLabel: {
    fontSize: '8px',
    fontWeight: 600,
    textAlign: 'center',
    color: theme.colors.gray[5],
    lineHeight: 1,
    marginBottom: 2,
  },
  meterRow: {
    display: 'flex',
    alignItems: 'center',
    gap: 4,
    marginBottom: 1,
  },
  channelTag: {
    fontSize: '7px',
    fontWeight: 700,
    color: theme.colors.gray[6],
    width: 12,
    textAlign: 'center',
  },
}));

type StereoChannelLevels = {
  left: { peak: number; rms: number };
  right: { peak: number; rms: number };
  peak: number; // Legacy mono compatibility
  rms: number; // Legacy mono compatibility
};

type ChannelVUMeterProps = {
  levels: StereoChannelLevels;
};

export const ChannelVUMeter = memo<ChannelVUMeterProps>(
  ({ levels }) => {
    const { classes } = useStyles();

    return (
      <Center>
        <div className={classes.vuContainer}>
          <Stack gap={1}>
            {/* Left Channel */}
            <div className={classes.meterRow}>
              <Text className={classes.channelTag}>L</Text>
              <VUMeter
                peakLevel={levels.left.peak}
                rmsLevel={levels.left.rms}
                height={10}
                width={250}
                vertical={false}
              />
            </div>
            {/* Right Channel */}
            <div className={classes.meterRow}>
              <Text className={classes.channelTag}>R</Text>
              <VUMeter
                peakLevel={levels.right.peak}
                rmsLevel={levels.right.rms}
                height={10}
                width={250}
                vertical={false}
              />
            </div>
          </Stack>
        </div>
      </Center>
    );
  },
  (prev, next) =>
    prev.levels.left.peak === next.levels.left.peak &&
    prev.levels.left.rms === next.levels.left.rms &&
    prev.levels.right.peak === next.levels.right.peak &&
    prev.levels.right.rms === next.levels.right.rms
);
