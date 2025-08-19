// Professional VU Meter component with optimized performance
import { memo } from 'react';
import { VUMeterProps, VU_METER_COLORS, VU_METER_ZONES } from '../../types';
import { audioCalculations } from '../../utils';

export const VUMeter = memo<VUMeterProps>(({ 
  peakLevel, 
  rmsLevel, 
  vertical = true, 
  height = 200,
  width = 20,
  showLabels = true 
}) => {
  // Convert levels to dB
  const dbPeak = peakLevel > 0 ? audioCalculations.linearToDb(peakLevel) : -60;
  const dbRms = rmsLevel > 0 ? audioCalculations.linearToDb(rmsLevel) : -60;
  
  // Convert dB to VU meter positions (0-1 range)
  const peakPosition = audioCalculations.dbToVuPosition(dbPeak);
  const rmsPosition = audioCalculations.dbToVuPosition(dbRms);

  const segments = 30;
  const segmentSize = vertical ? height / segments : width / segments;

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
      
      const segmentStyle = vertical ? {
        height: `${segmentSize}px`,
        width: '100%',
        backgroundColor: isLit ? colorClass : (hasRms ? `${colorClass}40` : VU_METER_COLORS.OFF),
        marginBottom: '1px'
      } : {
        width: `${segmentSize}px`,
        height: '100%',
        backgroundColor: isLit ? colorClass : (hasRms ? `${colorClass}40` : VU_METER_COLORS.OFF),
        marginRight: '1px'
      };

      return (
        <div
          key={i}
          className="transition-colors duration-75"
          style={segmentStyle}
        />
      );
    });
  };

  const renderLabels = () => {
    if (!showLabels) return null;

    const labels = [0, -6, -12, -18, -24, -30, -40, -60];
    
    return labels.map(db => {
      const position = audioCalculations.dbToVuPosition(db);
      const pixelPosition = vertical 
        ? height - (position * height)
        : position * width;
      
      const labelStyle = vertical ? {
        position: 'absolute' as const,
        top: `${pixelPosition}px`,
        right: '-25px',
        fontSize: '10px',
        color: '#9ca3af',
        transform: 'translateY(-50%)'
      } : {
        position: 'absolute' as const,
        left: `${pixelPosition}px`,
        bottom: '-20px',
        fontSize: '10px',
        color: '#9ca3af',
        transform: 'translateX(-50%)'
      };

      return (
        <div key={db} style={labelStyle}>
          {db === 0 ? '0' : db}
        </div>
      );
    });
  };

  const containerStyle = {
    width: vertical ? `${width}px` : `${width}px`,
    height: vertical ? `${height}px` : `${20}px`,
    backgroundColor: VU_METER_COLORS.BACKGROUND,
    borderRadius: '2px',
    padding: '2px',
    position: 'relative' as const
  };

  const segmentContainerClass = vertical 
    ? 'flex flex-col-reverse gap-0' 
    : 'flex flex-row gap-0';

  return (
    <div style={containerStyle} className="select-none">
      <div className={`${segmentContainerClass} h-full w-full`}>
        {renderSegments()}
      </div>
      {renderLabels()}
      
      {/* Peak hold indicator */}
      {peakPosition > 0.8 && (
        <div 
          className="absolute w-full h-0.5 bg-white opacity-80"
          style={{
            [vertical ? 'top' : 'left']: `${vertical 
              ? height - (peakPosition * height) 
              : peakPosition * width}px`
          }}
        />
      )}
    </div>
  );
}, (prevProps, nextProps) => 
  // Custom comparison for performance optimization
  prevProps.peakLevel === nextProps.peakLevel && 
  prevProps.rmsLevel === nextProps.rmsLevel &&
  prevProps.vertical === nextProps.vertical &&
  prevProps.height === nextProps.height &&
  prevProps.width === nextProps.width
);