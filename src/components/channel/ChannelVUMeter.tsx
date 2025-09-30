// Channel-specific stereo VU meter component with separate L/R channels
import { Text, Center, Box, Stack } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useMemo, useRef } from 'react';

import { useChannelLevels } from '../../hooks';
import { VU_METER_OPTIMIZATIONS } from '../../utils/performance-helpers';
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
  channelId: number;
};

export const ChannelVUMeter = memo<ChannelVUMeterProps>(
  ({ channelId }) => {
    const { classes } = useStyles();
    const levels = useChannelLevels(channelId);
    const previousLevelsRef = useRef<
      StereoChannelLevels & {
        meterElements?: React.ReactNode;
      }
    >({
      left: { peak: 0, rms: 0 },
      right: { peak: 0, rms: 0 },
      peak: 0,
      rms: 0,
    });

    // Check if levels have changed significantly enough to warrant re-render
    const levelsChanged = useMemo(() => {
      const prev = previousLevelsRef.current;
      const changed =
        !VU_METER_OPTIMIZATIONS.levelsEqual(prev.left, levels.left) ||
        !VU_METER_OPTIMIZATIONS.levelsEqual(prev.right, levels.right);

      if (changed) {
        previousLevelsRef.current = levels;
      }
      return changed;
    }, [levels]);

    // Memoize the VU meter elements to prevent unnecessary re-renders
    const meterElements = useMemo(() => {
      if (!levelsChanged && previousLevelsRef.current.left.peak > 0) {
        return previousLevelsRef.current.meterElements;
      }

      const elements = (
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
              showLabels={false} // Optimize by removing labels for channel meters
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
              showLabels={false} // Optimize by removing labels for channel meters
            />
          </div>
        </Stack>
      );

      // Cache the elements
      previousLevelsRef.current.meterElements = elements;
      return elements;
    }, [levels, classes.meterRow, classes.channelTag, levelsChanged]);

    return (
      <Center>
        <div className={classes.vuContainer}>{meterElements}</div>
      </Center>
    );
  },
  (prev, next) => {
    // Only re-render if channel ID changes
    return prev.channelId === next.channelId;
  }
);
