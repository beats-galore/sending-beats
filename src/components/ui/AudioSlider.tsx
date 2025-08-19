// Professional audio slider component
import { Stack, Text, Box, Slider } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useCallback, useMemo } from 'react';

import { useDebounce } from '../../utils/performance-helpers';

import type { AudioSliderProps } from '../../types';

const useStyles = createStyles((theme) => ({
  container: {
    alignItems: 'center',
    width: '100%',
  },
  
  label: {
    fontSize: '10px',
    color: theme.colors.gray[3],
    fontWeight: 500,
    textAlign: 'center',
  },
  
  valueDisplay: {
    fontSize: '10px',
    color: theme.colors.gray[4],
    fontFamily: 'monospace',
    minWidth: '4rem',
    textAlign: 'center',
  },
  
  sliderContainer: {
    height: 120,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    position: 'relative',
  },
}));

export const AudioSlider = memo<AudioSliderProps>(
  ({ label, value, min, max, step = 0.1, unit = '', onChange, disabled = false }) => {
    const { classes } = useStyles();
    
    // Debounce changes to prevent excessive updates during dragging
    const debouncedOnChange = useDebounce(onChange, 50);

    const handleChange = useCallback(
      (newValue: number) => {
        debouncedOnChange(newValue);
      },
      [debouncedOnChange]
    );

    // Format display value
    const displayValue = useMemo(() => 
      `${value.toFixed(step < 1 ? 1 : 0)}${unit}`,
      [value, step, unit]
    );

    // Create marks for the slider
    const marks = useMemo(() => {
      const marksArray = [
        { value: min, label: `${min}${unit}` },
        { value: max, label: `${max}${unit}` },
      ];
      
      // Add center mark for gain controls
      if (min < 0 && max > 0) {
        marksArray.splice(1, 0, { value: 0, label: '0' });
      }
      
      return marksArray;
    }, [min, max, unit]);

    return (
      <Stack gap="xs" className={classes.container}>
        {/* Label */}
        <Text className={classes.label}>{label}</Text>

        {/* Value display */}
        <Text className={classes.valueDisplay}>{displayValue}</Text>

        {/* Slider */}
        <Box className={classes.sliderContainer}>
          <Slider
            orientation="vertical"
            min={min}
            max={max}
            step={step}
            value={value}
            onChange={handleChange}
            disabled={disabled}
            marks={marks}
            size="sm"
            style={{ height: '100px' }}
          />
        </Box>
      </Stack>
    );
  },
  (prevProps, nextProps) =>
    prevProps.value === nextProps.value &&
    prevProps.disabled === nextProps.disabled &&
    prevProps.min === nextProps.min &&
    prevProps.max === nextProps.max
);
