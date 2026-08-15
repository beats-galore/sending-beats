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
 * How much of a source node is showing, smallest first.
 *
 * Which one a node is at follows from how big it has been made: a node shows as
 * much as it has room for, and nothing more. Shrunk to the bottom it is only a
 * name, a pair of thin meters and its mute and solo — small enough that a
 * canvas full of them still reads. Above that it is the whole card down to and
 * including the effects switch, and above that again the chain that switch
 * turns on.
 *
 * There is deliberately no rung between the card and the chain. The switch is
 * not something a card earns by being opened far enough: a source you can see
 * at all is one you can silence the processing on.
 */
export const ChannelExpansion = ['compact', 'collapsed', 'effects'] as const;
export type ChannelExpansion = (typeof ChannelExpansion)[number];

/**
 * How much of any other node is showing, smallest first.
 *
 * Buses, the broadcast and the tape all have the same three: shrunk to the one
 * reading that says what they are doing, shut, and open with their settings.
 */
export const NodeExpansion = ['compact', 'collapsed', 'expanded'] as const;
export type NodeExpansion = (typeof NodeExpansion)[number];

/** A hardware destination has nothing to open into, so it only shrinks. */
export const OutputExpansion = ['compact', 'collapsed'] as const;
export type OutputExpansion = (typeof OutputExpansion)[number];

/**
 * Which of the shared rungs a source is standing at.
 *
 * Pinned nodes are sized as a group, and a group made of sources, mixes and
 * destinations needs one vocabulary they all answer to. A source's own names
 * differ only in that its open rung is named for what it reveals.
 */
export const rungOf = (expansion: ChannelExpansion): NodeExpansion =>
  expansion === 'effects' ? 'expanded' : expansion;

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

/**
 * A run of sizes a node can be at, smallest first.
 *
 * Read forwards it answers "how big does this node have to be to show that",
 * which is what the condense and expand toggle needs; read backwards it answers
 * "how much does a node this big have room for", which is what makes dragging
 * one bigger reveal more of it. The two have to come from one place or a node
 * would toggle to a size that shows something else.
 */
const heightLadder = <T extends string>(
  width: number,
  order: readonly T[],
  heights: Record<T, number>
) => ({
  size: (expansion: T): Size => ({ width, height: heights[expansion] }),

  /** The most a box this tall has room for, never less than the smallest rung. */
  expansionFor: (size: Size): T =>
    order.reduce((fitting, rung) => (size.height >= heights[rung] ? rung : fitting), order[0]),
});

// Only the chain needs the wider card: it lays an equaliser and a compressor
// out side by side, where the switch above them fits the narrow one.
const channelWidth = (expansion: ChannelExpansion): number =>
  expansion === 'effects' ? source.widthExpanded : source.width;

const expansionHeight = (expansion: ChannelExpansion): number => {
  switch (expansion) {
    case 'compact':
      return source.heightCompact;
    case 'collapsed':
      return source.height;
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
 * Both axes count. The chain is laid out against the wider card, so a node made
 * tall but left narrow would have to reflow it rather than reveal it, and stays
 * shut instead.
 */
export const expansionFor = (variant: ChannelCardVariant, size: Size): ChannelExpansion => {
  const fits = (expansion: ChannelExpansion): boolean => {
    const needed = channelSize(variant, expansion);
    return size.width >= needed.width && size.height >= needed.height;
  };

  return ChannelExpansion.reduce<ChannelExpansion>(
    (fitting, expansion) => (fits(expansion) ? expansion : fitting),
    'compact'
  );
};

/**
 * The next rung the condense and expand toggle goes to.
 *
 * One button, so it walks up through the sizes and folds all the way back down
 * from the top. Every state is a press or two away, and the button can say
 * which way the next press goes rather than claiming a node is simply open or
 * shut when there are more than two answers.
 */
export const nextExpansion = <T extends string>(current: T, order: readonly T[]): T => {
  const next = order.indexOf(current) + 1;
  return next < order.length ? order[next] : order[0];
};

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
 * Distance from a bus node's top edge to one of its output port centres.
 *
 * Spread evenly down the edge, since nothing on the card lines up with them —
 * where a mix goes is written as one line rather than a row per destination.
 */
export const busPortOffset = (portIndex: number, portCount: number): number =>
  bus.portOffset + portIndex * portStep(portCount);

/**
 * Distance from a bus node's top edge to one of its input port centres.
 *
 * Level with the member row it belongs to, so the cable carrying a source lands
 * beside that source's name and its level. Reading what feeds a mix, and how
 * hard, is then one horizontal glance.
 */
export const busInputPortOffset = (portIndex: number): number =>
  bus.membersTop + portIndex * bus.memberRowHeight + bus.memberRowHeight / 2;

// None of these widen as they open, so unlike a source it is only height that
// decides how much of one there is room for.
//
// A mix is the exception to being sized by expansion alone: it lists what feeds
// it one row per member, so its rungs move down the canvas as sources are
// patched into it.
const busLadder = (members: number) =>
  heightLadder(bus.width, NodeExpansion, {
    compact: bus.heightCompact,
    collapsed: bus.heightBase + members * bus.memberRowHeight,
    expanded: bus.heightExpandedBase + members * bus.memberRowHeight,
  });

export const busSize = (expansion: NodeExpansion, members: number): Size =>
  busLadder(members).size(expansion);

export const busExpansionFor = (size: Size, members: number): NodeExpansion =>
  busLadder(members).expansionFor(size);

const castLadder = heightLadder(destination.width, NodeExpansion, {
  compact: destination.castHeightCompact,
  collapsed: destination.castHeight,
  expanded: destination.castHeightExpanded,
});

const tapeLadder = heightLadder(destination.width, NodeExpansion, {
  compact: destination.tapeHeightCompact,
  collapsed: destination.tapeHeight,
  expanded: destination.tapeHeightExpanded,
});

const outputLadder = heightLadder(destination.width, OutputExpansion, {
  compact: destination.outputHeightCompact,
  collapsed: destination.outputHeight,
});

export const castSize = castLadder.size;
export const castExpansionFor = castLadder.expansionFor;
export const tapeSize = tapeLadder.size;
export const tapeExpansionFor = tapeLadder.expansionFor;
export const outputSize = outputLadder.size;
export const outputExpansionFor = outputLadder.expansionFor;

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
