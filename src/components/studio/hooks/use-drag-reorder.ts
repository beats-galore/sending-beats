import { useCallback, useRef, useState } from 'react';

import type { PointerEvent as ReactPointerEvent } from 'react';

/**
 * Marks the element a grip belongs to.
 *
 * The grip is what you press, but the row is what moves, and how far a drag has
 * travelled is measured in rows. Put `{...reorderRow}` on the row.
 */
const ROW_ATTRIBUTE = 'data-drag-row';

/** Marks the element the rows live in, so their spacing can be measured. */
const LIST_ATTRIBUTE = 'data-drag-list';

/** Spread onto the row a grip lives inside. */
export const reorderRow = { [ROW_ATTRIBUTE]: true } as const;

/** Spread onto the element holding the rows. */
export const reorderList = { [LIST_ATTRIBUTE]: true } as const;

/**
 * What a drag is currently doing, for the list to draw itself with.
 *
 * `from` is where the row started and `to` is where it would land — the list
 * renders itself in the order it would end up in, so what you see during the
 * drag is the result rather than a hole and a floating copy.
 */
export type ReorderDrag<Id extends string = string> = {
  id: Id;
  from: number;
  to: number;
};

type DragReorderOptions<Id extends string> = {
  /** Committed on release, and only when the row actually moved. */
  onMove: (id: Id, toIndex: number) => void;
};

/**
 * Reordering a list by dragging its rows.
 *
 * Pointer events rather than HTML5 drag and drop: the webview intercepts drags
 * at the operating system's level for file drops, which leaves in-page dragging
 * unreliable. Pointers are unaffected by any of that, and they behave the same
 * on a trackpad as on a mouse.
 *
 * The row's own height is what decides where it would land — a drag of one row
 * height is a move of one place — so the list does not need to be measured and
 * a scrolled list behaves the same as one that is not.
 */
/**
 * How far apart two rows are, from the top of one to the top of the next.
 *
 * The pitch, not the row's own height. A list that puts something between its
 * rows — the queue's breakpoint seams do — is spaced further apart than its
 * rows are tall, and measuring the row would make a drag of two places count
 * as three.
 */
const rowPitch = (grip: HTMLElement): number => {
  const row = grip.closest<HTMLElement>(`[${ROW_ATTRIBUTE}]`);
  const list = grip.closest<HTMLElement>(`[${LIST_ATTRIBUTE}]`);
  const rows = list ? [...list.querySelectorAll<HTMLElement>(`[${ROW_ATTRIBUTE}]`)] : [];

  if (rows.length > 1) {
    const gap = rows[1].getBoundingClientRect().top - rows[0].getBoundingClientRect().top;
    if (gap > 0) {
      return gap;
    }
  }

  // One row, or a list that has not said where its rows are: nothing to move
  // past, so its own height is as good an answer as there is.
  return row?.getBoundingClientRect().height || 1;
};

export const useDragReorder = <Id extends string>({ onMove }: DragReorderOptions<Id>) => {
  const [drag, setDrag] = useState<ReorderDrag<Id> | null>(null);

  // Held in a ref as well as state: the move handler runs on `window` and needs
  // the current target without being rebuilt on every pixel.
  const live = useRef<ReorderDrag<Id> | null>(null);

  const start = useCallback(
    (event: ReactPointerEvent<HTMLElement>, id: Id, index: number, count: number) => {
      // The row is inside a card that selects on click and a canvas that clears
      // it; neither wants to hear about a drag.
      event.stopPropagation();
      event.preventDefault();

      const rowHeight = rowPitch(event.currentTarget);
      const startY = event.clientY;

      const begin: ReorderDrag<Id> = { id, from: index, to: index };
      live.current = begin;
      setDrag(begin);

      const handleMove = (moveEvent: PointerEvent) => {
        const moved = Math.round((moveEvent.clientY - startY) / rowHeight);
        const to = Math.min(count - 1, Math.max(0, index + moved));

        if (live.current && live.current.to !== to) {
          const next = { ...live.current, to };
          live.current = next;
          setDrag(next);
        }
      };

      const handleUp = () => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);

        const finished = live.current;
        live.current = null;
        setDrag(null);

        if (finished && finished.to !== finished.from) {
          onMove(finished.id, finished.to);
        }

        // A drag ends with a click on whatever ancestor the press and release
        // share, which would read as a click on the row. Swallow the one click
        // a drag produces.
        const swallow = (clickEvent: MouseEvent) => clickEvent.stopPropagation();
        window.addEventListener('click', swallow, { capture: true, once: true });
        setTimeout(() => window.removeEventListener('click', swallow, true), 0);
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
    },
    [onMove]
  );

  return { drag, start };
};

/**
 * A list in the order a drag would leave it.
 *
 * Applied while dragging so the rows move under the pointer, which is the whole
 * feedback — without it a drag is a row that highlights and nothing else.
 */
export const withDragApplied = <T,>(items: T[], drag: ReorderDrag<string> | null): T[] => {
  if (!drag || drag.from === drag.to) {
    return items;
  }

  const reordered = [...items];
  const [moved] = reordered.splice(drag.from, 1);
  reordered.splice(drag.to, 0, moved);
  return reordered;
};
