import { useCallback } from 'react';

import type { PointerEvent as ReactPointerEvent } from 'react';

type DragAxis = 'x' | 'y';

type DragValueOptions = {
  min: number;
  max: number;
  /** 'x' fills left to right, 'y' fills bottom to top. */
  axis?: DragAxis;
  onChange: (value: number) => void;
};

/**
 * Turns a track element into a scrubbable control.
 *
 * The returned handler reads the value from the pointer's position within the
 * track it is attached to, then keeps following the pointer on `window` until
 * release — so a drag that leaves the track keeps working, which is how every
 * fader in the mixer is expected to behave.
 */
export const useDragValue = ({ min, max, axis = 'x', onChange }: DragValueOptions) =>
  useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      const track = event.currentTarget.getBoundingClientRect();

      const apply = (point: { clientX: number; clientY: number }) => {
        const ratio =
          axis === 'x'
            ? (point.clientX - track.left) / track.width
            : 1 - (point.clientY - track.top) / track.height;
        onChange(min + Math.min(1, Math.max(0, ratio)) * (max - min));
      };

      apply(event);

      const handleMove = (moveEvent: PointerEvent) => apply(moveEvent);
      const handleUp = () => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
    },
    [min, max, axis, onChange]
  );

/** Position of `value` within `[min, max]`, as a CSS percentage string. */
export const percentOf = (value: number, min: number, max: number): string =>
  `${Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100))}%`;
