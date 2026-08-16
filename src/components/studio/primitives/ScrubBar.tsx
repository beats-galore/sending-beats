import { Box } from '@mantine/core';
import { useCallback, useState } from 'react';

import type { PointerEvent as ReactPointerEvent } from 'react';
import { color } from '../../../theme/tokens';

type ScrubBarProps = {
  /** Where the playhead is, in seconds. */
  position: number;
  /** How long the track runs, in seconds. Zero disables scrubbing. */
  length: number;
  /** The colour the played part reads in. */
  tint: string;
  height?: number;
  onSeek: (seconds: number) => void;
};

/**
 * The playhead, and a way to move it.
 *
 * Unlike a fader, this commits on release rather than on every move: seeking
 * tears down the decoder's buffers and rebuilds the resampler, so following the
 * pointer would ask for that several times per drag. Dragging shows where the
 * playhead is going and the release is what asks for it.
 */
export const ScrubBar = ({ position, length, tint, height = 3, onSeek }: ScrubBarProps) => {
  const [scrubbing, setScrubbing] = useState<number | null>(null);

  const startScrub = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      // Nodes select on click; scrubbing inside one must not also select it.
      event.stopPropagation();

      if (length <= 0) {
        return;
      }

      const track = event.currentTarget.getBoundingClientRect();
      const secondsAt = (clientX: number): number => {
        const ratio = (clientX - track.left) / track.width;
        return Math.min(1, Math.max(0, ratio)) * length;
      };

      setScrubbing(secondsAt(event.clientX));

      const handleMove = (moveEvent: PointerEvent) => setScrubbing(secondsAt(moveEvent.clientX));
      const handleUp = (upEvent: PointerEvent) => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);

        setScrubbing(null);
        onSeek(secondsAt(upEvent.clientX));

        // A drag ends with a click on whatever ancestor the press and the
        // release share, which would read as a click on the canvas and close
        // the open node. Swallow the one click a drag produces.
        const swallow = (clickEvent: MouseEvent) => clickEvent.stopPropagation();
        window.addEventListener('click', swallow, { capture: true, once: true });
        setTimeout(() => window.removeEventListener('click', swallow, true), 0);
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
    },
    [length, onSeek]
  );

  const shown = scrubbing ?? position;
  const progress = length > 0 ? Math.min(1, Math.max(0, shown / length)) : 0;

  return (
    <Box
      onPointerDown={startScrub}
      style={{
        flex: 1,
        minWidth: 0,
        height,
        background: color.line,
        borderRadius: Math.max(2, Math.round(height / 2)),
        overflow: 'hidden',
        cursor: length > 0 ? 'ew-resize' : 'default',
        touchAction: 'none',
      }}
    >
      <Box
        style={{
          width: `${progress * 100}%`,
          height: '100%',
          background: tint,
          // Matches the poll beat so the bar creeps rather than steps, but not
          // while dragging, where it has to keep up with the pointer.
          transition: scrubbing === null ? 'width 500ms linear' : undefined,
        }}
      />
    </Box>
  );
};
