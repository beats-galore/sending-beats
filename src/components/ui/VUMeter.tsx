// Professional VU Meter component with optimized performance
import { Box } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useMemo } from 'react';

import { VU_METER_COLORS, VU_METER_ZONES } from '../../types';
import { audioCalculations } from '../../utils';

import type { VUMeterProps } from '../../types';

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

export const VUMeter = memo<VUMeterProps>(
  ({ peakLevel, rmsLevel, vertical = true, height = 200, width = 20, showLabels = true }) => {
    const { classes } = useStyles();
    
    // Convert levels to dB
    const dbPeak = peakLevel > 0 ? audioCalculations.linearToDb(peakLevel) : -60;
    const dbRms = rmsLevel > 0 ? audioCalculations.linearToDb(rmsLevel) : -60;

    // Convert dB to VU meter positions (0-1 range)
    const peakPosition = audioCalculations.dbToVuPosition(dbPeak);
    const rmsPosition = audioCalculations.dbToVuPosition(dbRms);

    const segments = 30;
    const segmentSize = vertical ? height / segments : width / segments;

    // Memoize container style
    const containerStyle = useMemo(() => ({
      width: `${width}px`,
      height: vertical ? `${height}px` : '20px',
    }), [width, height, vertical]);

    const renderSegments = () => {
      return Array.from({ length: segments }, (_, i) => {
        const segmentValue = (i + 1) / segments;
        const isLit = segmentValue <= peakPosition;
        const isRmsLit = segmentValue <= rmsPosition;

        // Color coding based on level zones
        let colorClass: string = VU_METER_COLORS.OFF;
        if (isLit) {
          if (segmentValue < VU_METER_ZONES.GREEN_THRESHOLD) {
            colorClass = VU_METER_COLORS.GREEN;
          } else if (segmentValue < VU_METER_ZONES.YELLOW_THRESHOLD) {
            colorClass = VU_METER_COLORS.YELLOW;
          } else {
            colorClass = VU_METER_COLORS.RED;
          }
        }

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

        return (
          <div 
            key={i} 
            className={classes.segment} 
            style={segmentStyle} 
          />
        );
      });
    };

    const renderLabels = () => {
      if (!showLabels) return null;

      const labels = [0, -6, -12, -18, -24, -30, -40, -60];

      return labels.map((db) => {
        const position = audioCalculations.dbToVuPosition(db);
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
    };

    // Memoize peak hold style
    const peakHoldStyle = useMemo(() => ({
      [vertical ? 'top' : 'left']: `${
        vertical ? height - peakPosition * height : peakPosition * width
      }px`,
    }), [vertical, height, width, peakPosition]);

    return (
      <Box 
        className={classes.container}
        style={containerStyle}
      >
        <div 
          className={`${classes.segmentContainer} ${
            vertical ? classes.segmentContainerVertical : classes.segmentContainerHorizontal
          }`}
        >
          {renderSegments()}
        </div>
        
        {renderLabels()}

        {/* Peak hold indicator */}
        {peakPosition > 0.8 && (
          <div
            className={classes.peakHold}
            style={peakHoldStyle}
          />
        )}
      </Box>
    );
  },
  (prevProps, nextProps) =>
    // Custom comparison for performance optimization
    prevProps.peakLevel === nextProps.peakLevel &&
    prevProps.rmsLevel === nextProps.rmsLevel &&
    prevProps.vertical === nextProps.vertical &&
    prevProps.height === nextProps.height &&
    prevProps.width === nextProps.width
);
