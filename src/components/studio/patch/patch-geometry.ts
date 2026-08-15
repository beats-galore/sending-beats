// Coordinate maths for the patch canvas.
//
// Nodes are placed absolutely and joined by bezier cables, so positions have to
// be computed rather than laid out. Everything here is pure: given the channel
// list and what is selected, it returns how much room each node asks for and
// where a column puts it.
//
// This is the arrangement the canvas would make on its own. `patch-rects` puts
// hand-placed nodes on top of it, so what is here is the default rather than
// the final word.
import { layout } from '../../../theme/layout';
import type { Port } from './patch-layout';

const { source, bus, destination, patch } = layout;

/**
 * How much of a source node is showing. The focused node drops its effects
 * chain when the channel's effects are switched off, so it takes less room than
 * a focused node whose chain is live.
 */
export const ChannelExpansion = ['collapsed', 'inspector', 'effects'] as const;
export type ChannelExpansion = (typeof ChannelExpansion)[number];

/**
 * Which card a source is drawn as.
 *
 * An application the backend can read a track from stands taller, because its
 * card carries a track readout the hardware card has no use for. The variant
 * follows the patched source rather than whether something is playing, so a
 * node keeps its size when playback stops.
 */
export const ChannelCardVariant = ['device', 'app'] as const;
export type ChannelCardVariant = (typeof ChannelCardVariant)[number];

/** Everything about a source node that decides how much room it takes. */
export type ChannelLayout = {
  variant: ChannelCardVariant;
  expansion: ChannelExpansion;
};

export const channelWidth = (expansion: ChannelExpansion): number =>
  expansion === 'collapsed' ? source.width : source.widthExpanded;

const expansionHeight = (expansion: ChannelExpansion): number => {
  switch (expansion) {
    case 'collapsed':
      return source.height;
    case 'inspector':
      return source.heightInspector;
    case 'effects':
      return source.heightExpanded;
  }
};

export const channelHeight = ({ variant, expansion }: ChannelLayout): number =>
  expansionHeight(expansion) + (variant === 'app' ? source.trackReadoutHeight : 0);

/**
 * Top edge of each item in a column, given how much room each one takes.
 *
 * A node that opens pushes the ones below it down rather than overlapping them,
 * which is what every column on the canvas does. Heights come in resolved, so a
 * node the user made taller pushes its neighbours the same way an opened one
 * does.
 */
export const stackTops = (heights: number[], top: number, gap: number): number[] => {
  const tops: number[] = [];
  let next = top;

  for (const height of heights) {
    tops.push(next);
    next += height + gap;
  }

  return tops;
};

/** Distance from a source node's top edge to its output port centre. */
export const channelPortOffset = source.portOffset;

/** Ports are spread evenly but never further apart than `portSpacing`. */
const portStep = (count: number): number =>
  Math.min(bus.portSpacing, bus.portSpan / Math.max(count, 1));

/**
 * Distance from a bus node's top edge to one of its port centres.
 *
 * Members are listed on the node in the same order the ports run down its edge,
 * so a cable lands beside the tile naming what it carries.
 */
export const busPortOffset = (portIndex: number, portCount: number): number =>
  bus.portOffset + portIndex * portStep(portCount);

/**
 * How tall a bus node is drawn.
 *
 * The middle column flows the way the source column does: the node that opens
 * pushes the ones below it down rather than overlapping them.
 */
export const busHeight = (expanded: boolean): number =>
  expanded ? bus.heightExpanded : bus.height;

/**
 * Which destination card is opened, if any.
 *
 * The right column flows the way the source column does: a card that opens
 * pushes everything below it down rather than overlapping it.
 */
export type DestinationFocus = 'cast' | 'tape' | null;

export const castHeight = (focus: DestinationFocus): number =>
  focus === 'cast' ? destination.castHeightExpanded : destination.castHeight;

export const tapeHeight = (focus: DestinationFocus): number =>
  focus === 'tape' ? destination.tapeHeightExpanded : destination.tapeHeight;

/**
 * A cable between two ports: horizontal at both ends, curving in the middle, so
 * it leaves and lands square against the node it connects to.
 *
 * The reach grows with the run when a cable has to double back — a source
 * dragged to the right of the mix it feeds still leaves its node rightwards and
 * still lands square, and without the extra reach the two ends would meet in a
 * knot rather than a legible loop.
 */
export const cablePath = (from: Port, to: Port): string => {
  const reach = Math.max(patch.cableControlReach, (from.x - to.x) / 2);

  return (
    `M${from.x},${from.y} C${from.x + reach},${from.y} ` +
    `${to.x - reach},${to.y} ${to.x},${to.y}`
  );
};
