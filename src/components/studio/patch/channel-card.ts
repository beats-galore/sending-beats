import type { MouseEventHandler, PointerEventHandler, ReactNode } from 'react';

import type { NodeRect } from './patch-layout';

/**
 * What every source card needs regardless of variant.
 *
 * The variants differ in what they draw, not in how they are addressed or how
 * they are placed — the canvas has already resolved the box, so a card only has
 * to fill it.
 */
export type ChannelCardProps = {
  /** Box in canvas coordinates, with anything the user arranged applied. */
  rect: NodeRect;
  header: ReactNode;
  children: ReactNode;
  ports?: ReactNode;
  borderColor?: string;
  selected?: boolean;
  /** Draws the node above every other, for the one last pressed. */
  raised?: boolean;
  /** Called on any press, to bring the node forward out of a stack. */
  onPress?: PointerEventHandler<HTMLDivElement>;
  onClick?: MouseEventHandler<HTMLDivElement>;
  /** Picks the node up. The title bar is the grip. */
  onGrab?: PointerEventHandler<HTMLDivElement>;
  /** Resizes the node. Draws the corner grip when given. */
  onResize?: PointerEventHandler<HTMLDivElement>;
};
