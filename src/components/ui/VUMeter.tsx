// Professional VU Meter component with optimized performance
import { Box } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useMemo, useRef, useCallback } from 'react';

import { VU_METER_COLORS, VU_METER_ZONES } from '../../types';
import type { VUMeterProps } from '../../types';
import { audioCalculations } from '../../utils';


const useStyles = createStyles((theme) => ({
  container: {
    backgroundColor: VU_METER_COLORS.BACKGROUND,
    borderRadius: '2px',
    padding: '2px',
    position: 'relative',
    userSelect: 'none',
  },

  segmentContainer: {
    height: '100%',
    width: '100%',
  },

  segmentContainerVertical: {
    display: 'flex',
    flexDirection: 'column-reverse',
    gap: 0,
  },

  segmentContainerHorizontal: {
    display: 'flex',
    flexDirection: 'row',
    gap: 0,
  },

  segment: {
    transition: 'background-color 75ms',
  },

  label: {
    position: 'absolute',
    fontSize: '10px',
    color: '#9ca3af',
  },

  peakHold: {
    position: 'absolute',
    width: '100%',
    height: '2px',
    backgroundColor: 'white',
    opacity: 0.8,
  },
}));

// Memoize expensive calculations outside component
const memoizeDbConversion = (() => {
  const cache = new Map<number, number>();
  return (level: number): number => {
    if (!cache.has(level)) {
      cache.set(level, level > 0 ? audioCalculations.linearToDb(level) : -60);
      // Prevent memory leaks - keep cache size reasonable
      if (cache.size > 1000) {
        const firstKey = cache.keys().next().value;
        if (firstKey !== undefined) {
          cache.delete(firstKey);
        }
      }
    }
    return cache.get(level)!;
  };
})();

const memoizeVuPosition = (() => {
  const cache = new Map<number, number>();
  return (dbLevel: number): number => {
    if (!cache.has(dbLevel)) {
      cache.set(dbLevel, audioCalculations.dbToVuPosition(dbLevel));
      if (cache.size > 1000) {
        const firstKey = cache.keys().next().value;
        if (firstKey !== undefined) {
          cache.delete(firstKey);
        }
      }
    }
    return cache.get(dbLevel)!;
  };
})();

export const VUMeter = memo<VUMeterProps>(
  ({ peakLevel, rmsLevel, vertical = true, height = 200, width = 20, showLabels = true }) => {
    const { classes } = useStyles();
    const previousLevelsRef = useRef<{
      peakLevel: number;
      rmsLevel: number;
      segmentElements?: React.ReactNode[];
    }>({ peakLevel: 0, rmsLevel: 0 });

    // Use memoized conversion functions for better performance
    const dbPeak = useMemo(() => memoizeDbConversion(peakLevel), [peakLevel]);
    const dbRms = useMemo(() => memoizeDbConversion(rmsLevel), [rmsLevel]);

    // Convert dB to VU meter positions (0-1 range) - memoized
    const peakPosition = useMemo(() => memoizeVuPosition(dbPeak), [dbPeak]);
    const rmsPosition = useMemo(() => memoizeVuPosition(dbRms), [dbRms]);

    const segments = 30;
    const segmentSize = useMemo(
      () => (vertical ? height / segments : width / segments),
      [vertical, height, width, segments]
    );

    // Memoize container style
    const containerStyle = useMemo(
      () => ({
        width: `${width}px`,
        height: vertical ? `${height}px` : '20px',
      }),
      [width, height, vertical]
    );

    // Skip expensive render if levels haven't changed significantly
    const levelsChanged = useMemo(() => {
      const prev = previousLevelsRef.current;
      const threshold = 0.001; // Only re-render if change is > 0.1%
      const changed =
        Math.abs(peakLevel - prev.peakLevel) > threshold ||
        Math.abs(rmsLevel - prev.rmsLevel) > threshold;

      if (changed) {
        previousLevelsRef.current = { peakLevel, rmsLevel };
      }
      return changed;
    }, [peakLevel, rmsLevel]);

    // Memoize color calculation
    const getSegmentColor = useCallback((segmentValue: number): string => {
      if (segmentValue < VU_METER_ZONES.GREEN_THRESHOLD) {
        return VU_METER_COLORS.GREEN;
      } else if (segmentValue < VU_METER_ZONES.YELLOW_THRESHOLD) {
        return VU_METER_COLORS.YELLOW;
      }
      return VU_METER_COLORS.RED;
    }, []);

    // Memoize segments rendering - only re-render if positions change significantly
    const segmentElements = useMemo(() => {
      if (!levelsChanged && previousLevelsRef.current.peakLevel > 0) {
        return previousLevelsRef.current.segmentElements || [];
      }

      const elements = Array.from({ length: segments }, (_, i) => {
        const segmentValue = (i + 1) / segments;
        const isLit = segmentValue <= peakPosition;
        const isRmsLit = segmentValue <= rmsPosition;

        // Get color for this segment
        const colorClass = isLit ? getSegmentColor(segmentValue) : VU_METER_COLORS.OFF;

        // Add RMS indication as slightly dimmed background
        const hasRms = isRmsLit && !isLit;

        const segmentStyle = vertical
          ? {
              height: `${segmentSize}px`,
              width: '100%',
              backgroundColor: isLit
                ? colorClass
                : hasRms
                  ? `${colorClass}40`
                  : VU_METER_COLORS.OFF,
              marginBottom: '1px',
            }
          : {
              width: `${segmentSize}px`,
              height: '100%',
              backgroundColor: isLit
                ? colorClass
                : hasRms
                  ? `${colorClass}40`
                  : VU_METER_COLORS.OFF,
              marginRight: '1px',
            };

        return <div key={i} className={classes.segment} style={segmentStyle} />;
      });

      // Cache the rendered elements for reuse
      (previousLevelsRef.current as any).segmentElements = elements;
      return elements;
    }, [
      segments,
      peakPosition,
      rmsPosition,
      vertical,
      segmentSize,
      getSegmentColor,
      classes.segment,
      levelsChanged,
    ]);

    // Memoize labels rendering (only depends on dimensions and orientation)
    const labelElements = useMemo(() => {
      if (!showLabels) {return null;}

      const labels = [0, -6, -12, -18, -24, -30, -40, -60];

      return labels.map((db) => {
        const position = memoizeVuPosition(db);
        const pixelPosition = vertical ? height - position * height : position * width;

        const labelStyle = vertical
          ? {
              top: `${pixelPosition}px`,
              right: '-25px',
              transform: 'translateY(-50%)',
            }
          : {
              left: `${pixelPosition}px`,
              bottom: '-20px',
              transform: 'translateX(-50%)',
            };

        return (
          <div key={db} className={classes.label} style={labelStyle}>
            {db === 0 ? '0' : db}
          </div>
        );
      });
    }, [showLabels, vertical, height, width, classes.label]);

    // Memoize peak hold style
    const peakHoldStyle = useMemo(
      () => ({
        [vertical ? 'top' : 'left']: `${
          vertical ? height - peakPosition * height : peakPosition * width
        }px`,
      }),
      [vertical, height, width, peakPosition]
    );

    return (
      <Box className={classes.container} style={containerStyle}>
        <div
          className={`${classes.segmentContainer} ${
            vertical ? classes.segmentContainerVertical : classes.segmentContainerHorizontal
          }`}
        >
          {segmentElements}
        </div>

        {labelElements}

        {/* Peak hold indicator */}
        {peakPosition > 0.8 && <div className={classes.peakHold} style={peakHoldStyle} />}
      </Box>
    );
  },
  (prevProps, nextProps) => {
    // Performance-optimized comparison with threshold
    const threshold = 0.001; // Only re-render if change is > 0.1%
    return (
      Math.abs(prevProps.peakLevel - nextProps.peakLevel) < threshold &&
      Math.abs(prevProps.rmsLevel - nextProps.rmsLevel) < threshold &&
      prevProps.vertical === nextProps.vertical &&
      prevProps.height === nextProps.height &&
      prevProps.width === nextProps.width &&
      prevProps.showLabels === nextProps.showLabels
    );
  }
);
