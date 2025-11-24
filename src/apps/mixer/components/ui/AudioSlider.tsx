//  audio slider component
import { Stack, Text, Box, Slider } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useCallback, useMemo } from 'react';

import type { AudioSliderProps } from '../../types';
import { useDebounce } from '../../utils/performance-helpers';

const useStyles = createStyles((theme) => ({
  container: {
    alignItems: 'center',
    width: '100%',
    minWidth: '120px',
    flex: 1,
  },

  label: {
    fontSize: '12px',
    color: theme.colors.blue[4],
    fontWeight: 600,
    textAlign: 'center',
  },

  valueDisplay: {
    fontSize: '14px',
    color: theme.colors.gray[1],
    fontFamily: 'monospace',
    minWidth: '4rem',
    textAlign: 'center',
    backgroundColor: theme.colors.dark[8],
    padding: '4px 8px',
    borderRadius: '4px',
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  sliderContainer: {
    height: 140,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    position: 'relative',
    padding: '10px',
    width: '100%',
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
    const displayValue = useMemo(
      () => `${value.toFixed(step < 1 ? 1 : 0)}${unit}`,
      [value, step, unit]
    );

    // Create simplified marks for the slider - only show key values
    const marks = useMemo(() => {
      const marksArray: { value: number; label: string }[] = [];

      // For gain controls, only show 0dB mark
      if (min < 0 && max > 0 && unit === 'dB') {
        marksArray.push({ value: 0, label: '0' });
      } else {
        // For other controls, show min and max
        marksArray.push({ value: min, label: `${min}` }, { value: max, label: `${max}` });
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
            min={min}
            max={max}
            step={step}
            value={value}
            onChange={handleChange}
            disabled={disabled}
            marks={marks}
            size="md"
            style={{ height: '120px', width: '100%' }}
            styles={{
              mark: { fontSize: '10px', color: '#9ca3af' },
              markLabel: { fontSize: '10px', color: '#9ca3af', marginLeft: '8px' },
            }}
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
