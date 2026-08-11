import { Box } from '@mantine/core';

import type { PointerEvent as ReactPointerEvent } from 'react';
import { color } from '../../../theme/tokens';
import { percentOf, useDragValue } from '../hooks/use-drag-value';


export const DragTone = ['accent', 'warn', 'muted', 'text'] as const;
export type DragTone = (typeof DragTone)[number];

type DragBarProps = {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  height?: number;
  tone?: DragTone;
  /** Width and height of the grip. Omit for a bare fill with no grip. */
  knob?: [number, number] | null;
  /** Draws a tick at the midpoint — used by pan, where centre is the default. */
  centerMark?: boolean;
};

const TONE_COLOR: Record<DragTone, string> = {
  accent: color.acc,
  warn: color.warn,
  muted: color.textFaintest,
  text: color.text,
};

/** A horizontal scrub track: gain, pan, thresholds and every other continuous value. */
export const DragBar = ({
  value,
  min,
  max,
  onChange,
  height = 4,
  tone = 'accent',
  knob = null,
  centerMark = false,
}: DragBarProps) => {
  const startDrag = useDragValue({ min, max, onChange });
  const position = percentOf(value, min, max);
  const radius = Math.max(2, Math.round(height / 2));

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    // Nodes select on click; scrubbing a control inside one must not also select it.
    event.stopPropagation();
    startDrag(event);
  };

  return (
    <Box
      onPointerDown={handlePointerDown}
      style={{
        flex: 1,
        minWidth: 0,
        height,
        background: color.line,
        borderRadius: radius,
        position: 'relative',
        cursor: 'ew-resize',
        touchAction: 'none',
      }}
    >
      {centerMark && (
        <Box
          style={{
            position: 'absolute',
            left: '50%',
            top: -3,
            bottom: -3,
            width: 1,
            background: color.textFaintest,
          }}
        />
      )}
      <Box
        style={{
          position: 'absolute',
          left: 0,
          top: 0,
          bottom: 0,
          width: position,
          borderRadius: radius,
          background: TONE_COLOR[tone],
        }}
      />
      {knob && (
        <Box
          style={{
            position: 'absolute',
            left: position,
            top: '50%',
            transform: 'translate(-50%, -50%)',
            width: knob[0],
            height: knob[1],
            background: color.text,
            borderRadius: 'var(--mantine-radius-xs)',
            boxShadow: 'var(--mantine-shadow-xs)',
          }}
        />
      )}
    </Box>
  );
};
