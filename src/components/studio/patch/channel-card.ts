import type { MouseEventHandler, ReactNode } from 'react';

import type { ChannelExpansion } from './patch-geometry';

/**
 * What every source card needs regardless of variant.
 *
 * The variants differ in what they draw and how tall they stand, not in how
 * they are addressed, so the canvas can place any of them the same way.
 */
export type ChannelCardProps = {
  expansion: ChannelExpansion;
  /** Top edge in canvas coordinates. */
  top: number;
  header: ReactNode;
  children: ReactNode;
  ports?: ReactNode;
  borderColor?: string;
  selected?: boolean;
  onClick?: MouseEventHandler<HTMLDivElement>;
};
