import { Box } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useMemo, useRef } from 'react';
import { VU_METER_OPTIMIZATIONS } from '../../utils/performance-helpers';

type VUMeterProps = {
  level: number;
};

const useStyles = createStyles((theme) => ({
  vuMeter: {
    height: 80,
    backgroundColor: theme.colors.dark[8],
    borderRadius: theme.radius.sm,
    padding: theme.spacing.xs,
    display: 'flex',
    alignItems: 'flex-end',
    gap: 2,
  },

  vuBar: {
    flex: 1,
    backgroundColor: theme.colors.gray[6],
    borderRadius: 1,
    transition: 'all 0.1s ease',
    minHeight: 2,
  },

  vuBarActive: {
    backgroundColor: theme.colors.blue[5],
  },
}));

export const VUMeter = memo<VUMeterProps>(({ level }) => {
  const { classes } = useStyles();
  const previousLevelRef = useRef(0);
  
  // Skip expensive render if level hasn't changed significantly
  const levelChanged = useMemo(() => {
    const threshold = VU_METER_OPTIMIZATIONS.RENDER_THRESHOLD * 255; // Scale threshold to 0-255 range
    const changed = Math.abs(level - previousLevelRef.current) > threshold;
    if (changed) {
      previousLevelRef.current = level;
    }
    return changed;
  }, [level]);

  const normalizedLevel = useMemo(() => Math.max(0, Math.min(1, level / 255)), [level]);
  const barCount = 20;
  const activeBars = useMemo(() => Math.floor(normalizedLevel * barCount), [normalizedLevel, barCount]);

  const getBarColor = useMemo(() => (index: number, isActive: boolean) => {
    if (!isActive) return '#495057';

    if (index > barCount * 0.8) return '#fa5252';
    if (index > barCount * 0.6) return '#fd7e14';
    return '#339af0';
  }, [barCount]);

  const barHeight = useMemo(() => `${Math.max(10, normalizedLevel * 100)}%`, [normalizedLevel]);

  // Memoize bar elements to prevent unnecessary re-renders
  const barElements = useMemo(() => {
    if (!levelChanged && previousLevelRef.current > 0) {
      return (previousLevelRef.current as any).barElements || [];
    }

    const elements = Array.from({ length: barCount }, (_, i) => (
      <Box
        key={i}
        className={`${classes.vuBar} ${i < activeBars ? classes.vuBarActive : ''}`}
        style={{
          height: barHeight,
          backgroundColor: getBarColor(i, i < activeBars),
        }}
      />
    ));

    // Cache the rendered elements
    (previousLevelRef.current as any).barElements = elements;
    return elements;
  }, [barCount, activeBars, barHeight, getBarColor, classes.vuBar, classes.vuBarActive, levelChanged]);

  return (
    <Box className={classes.vuMeter}>
      {barElements}
    </Box>
  );
}, (prevProps, nextProps) => {
  // Optimized comparison - only re-render if change is significant
  const threshold = VU_METER_OPTIMIZATIONS.RENDER_THRESHOLD * 255;
  return Math.abs(prevProps.level - nextProps.level) < threshold;
});

VUMeter.displayName = 'VUMeter';
