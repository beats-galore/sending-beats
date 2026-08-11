import { Box, Stack, Text } from '@mantine/core';

import type { PointerEvent as ReactPointerEvent } from 'react';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { percentOf, useDragValue } from '../hooks/use-drag-value';


type DragColumnProps = {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  label: string;
  /** Formatted reading shown above the fader. */
  display: string;
  /** Colours the reading — boosted, cut or flat. */
  displayTone?: string;
};

/** A vertical fader with its reading above and its band name below. Used by the EQ. */
export const DragColumn = ({
  value,
  min,
  max,
  onChange,
  label,
  display,
  displayTone = color.textDim,
}: DragColumnProps) => {
  const startDrag = useDragValue({ min, max, axis: 'y', onChange });

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.stopPropagation();
    startDrag(event);
  };

  return (
    <Stack align="center" gap="xs" style={{ flex: 'none' }}>
      <Text size="2xs" c={displayTone}>
        {display}
      </Text>
      <Box
        onPointerDown={handlePointerDown}
        style={{
          width: 5,
          flex: 1,
          background: color.line,
          borderRadius: 3,
          position: 'relative',
          cursor: 'ns-resize',
          touchAction: 'none',
        }}
      >
        <Box
          style={{
            position: 'absolute',
            left: '50%',
            bottom: percentOf(value, min, max),
            transform: 'translate(-50%, 50%)',
            width: 24,
            height: 11,
            background: color.text,
            borderRadius: 'var(--mantine-radius-xs)',
            boxShadow: 'var(--mantine-shadow-xs)',
          }}
        />
      </Box>
      <Text size="3xs" c={color.textFaint} style={{ letterSpacing: layout.tracking.wider }}>
        {label}
      </Text>
    </Stack>
  );
};
