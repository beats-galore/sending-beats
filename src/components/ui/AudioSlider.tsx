// Professional audio slider component
import React, { memo, useCallback } from 'react';
import { AudioSliderProps } from '../../types';
import { useDebounce } from '../../utils/performance-helpers';

export const AudioSlider = memo<AudioSliderProps>(({
  label,
  value,
  min,
  max,
  step = 0.1,
  unit = '',
  onChange,
  disabled = false
}) => {
  // Debounce changes to prevent excessive updates during dragging
  const debouncedOnChange = useDebounce(onChange, 50);

  const handleChange = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseFloat(event.target.value);
    debouncedOnChange(newValue);
  }, [debouncedOnChange]);

  // Calculate percentage for visual representation
  const percentage = ((value - min) / (max - min)) * 100;
  
  // Format display value
  const displayValue = `${value.toFixed(step < 1 ? 1 : 0)}${unit}`;

  return (
    <div className="flex flex-col items-center space-y-2 w-full">
      {/* Label */}
      <label className="text-xs text-gray-300 font-medium text-center">
        {label}
      </label>
      
      {/* Value display */}
      <div className="text-xs text-gray-400 font-mono min-w-[4rem] text-center">
        {displayValue}
      </div>
      
      {/* Slider container */}
      <div className="relative w-full h-32 flex items-center justify-center">
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={handleChange}
          disabled={disabled}
          className="slider-vertical h-28 w-1"
          style={{
            writingMode: 'vertical-lr' as any,
            WebkitAppearance: 'slider-vertical',
          }}
        />
        
        {/* Custom slider track for better visual */}
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="w-1 h-28 bg-gray-700 rounded-full relative overflow-hidden">
            {/* Active track */}
            <div 
              className="absolute bottom-0 w-full bg-blue-500 transition-all duration-150 rounded-full"
              style={{ height: `${percentage}%` }}
            />
            
            {/* Center mark (for gain controls) */}
            {min < 0 && max > 0 && (
              <div 
                className="absolute w-2 h-0.5 bg-gray-400 left-1/2 transform -translate-x-1/2"
                style={{ 
                  bottom: `${((0 - min) / (max - min)) * 100}%`,
                  transform: 'translateX(-50%) translateY(50%)'
                }}
              />
            )}
          </div>
        </div>
        
        {/* Tick marks */}
        <div className="absolute right-2 h-28 flex flex-col justify-between text-xs text-gray-500">
          <span>{max}{unit}</span>
          {min < 0 && max > 0 && <span>0</span>}
          <span>{min}{unit}</span>
        </div>
      </div>
    </div>
  );
}, (prevProps, nextProps) =>
  prevProps.value === nextProps.value &&
  prevProps.disabled === nextProps.disabled &&
  prevProps.min === nextProps.min &&
  prevProps.max === nextProps.max
);