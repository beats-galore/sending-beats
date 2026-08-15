// Coordinate maths for the patch canvas.
//
// Nodes are placed absolutely and joined by bezier cables, so positions have to
// be computed rather than laid out. Everything here is pure: it says how much
// room a node needs to show a given amount of itself, how much it is showing at
// the size it has, and where a column puts a run of them.
//
// Sizes go both ways deliberately. `expansionFor` reads a box and says what
// fits in it, which is what makes dragging a node bigger reveal more of it;
// `channelSize` and friends answer the same question backwards, which is what
// the condense and expand toggle uses to jump straight to a size.
//
// This is the arrangement the canvas would make on its own. `patch-rects` puts
// hand-placed nodes on top of it, so what is here is the default rather than
// the final word.
import { layout } from '../../../theme/layout';
import type { Port, Size } from './patch-layout';

const { source, bus, destination, patch } = layout;

/**
 * How much of a source node is showing.
 *
 * Which one a node is at follows from how big it has been made: a node shows as
 * much as it has room for, and nothing more. The effects chain needs the most
 * room, the inspector alone needs less, and below that a node is only its
 * meters and its trim.
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

const channelWidth = (expansion: ChannelExpansion): number =>
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

/** How big a source node has to be to show a given amount of itself. */
export const channelSize = (variant: ChannelCardVariant, expansion: ChannelExpansion): Size => ({
  width: channelWidth(expansion),
  // An application card carries a track readout the hardware card has no use
  // for, so it needs that much more room to show the same amount of itself.
  height: expansionHeight(expansion) + (variant === 'app' ? source.trackReadoutHeight : 0),
});

/**
 * As much of a source as the box it has been given can actually show.
 *
 * Both axes count. The inspector and the chain are laid out against the wider
 * card, so a node made tall but left narrow would have to reflow them rather
 * than reveal them, and stays shut instead.
 */
export const expansionFor = (variant: ChannelCardVariant, size: Size): ChannelExpansion => {
  const fits = (expansion: ChannelExpansion): boolean => {
    const needed = channelSize(variant, expansion);
    return size.width >= needed.width && size.height >= needed.height;
  };

  return fits('effects') ? 'effects' : fits('inspector') ? 'inspector' : 'collapsed';
};

/**
 * How far a source opens when it is toggled open.
 *
 * As far as it has something to show: a channel with its effects switched off
 * has no chain, so opening it to the chain's height would only add dead space.
 */
export const openExpansion = (effectsEnabled: boolean): ChannelExpansion =>
  effectsEnabled ? 'effects' : 'inspector';

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
 * The nodes with only one thing to reveal.
 *
 * A bus gains its metering column and stats, the broadcast its transmitter, the
 * tape its output settings. None of them widen to do it, so unlike a source it
 * is only height that decides whether there is room.
 */
const sizeAt = (width: number, shut: number, open: number) => ({
  size: (expanded: boolean): Size => ({ width, height: expanded ? open : shut }),
  expandedFor: (size: Size): boolean => size.height >= open,
});

const busGeometry = sizeAt(bus.width, bus.height, bus.heightExpanded);
const castGeometry = sizeAt(
  destination.width,
  destination.castHeight,
  destination.castHeightExpanded
);
const tapeGeometry = sizeAt(
  destination.width,
  destination.tapeHeight,
  destination.tapeHeightExpanded
);

export const busSize = busGeometry.size;
export const busExpandedFor = busGeometry.expandedFor;
export const castSize = castGeometry.size;
export const castExpandedFor = castGeometry.expandedFor;
export const tapeSize = tapeGeometry.size;
export const tapeExpandedFor = tapeGeometry.expandedFor;

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
