// Coordinate maths for the patch canvas.
//
// Nodes are placed absolutely and joined by bezier cables, so positions have to
// be computed rather than laid out. Everything here is pure: given the channel
// list and what is selected, it returns where each node and port sits.
import { layout } from '../../../theme/layout';

const { source, bus, destination, canvas, patch } = layout;

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

/** Top edge of the node at `index`, given how much room each node before it takes. */
export const channelTop = (index: number, layouts: ChannelLayout[]): number =>
  layouts
    .slice(0, index)
    .reduce<number>((top, channel) => top + channelHeight(channel) + source.gap, source.top);

/** Total height of the source column including the trailing gap. */
export const sourceStackHeight = (layouts: ChannelLayout[]): number =>
  layouts.reduce<number>((total, channel) => total + channelHeight(channel) + source.gap, 0);

/** Where a channel's output port sits, in canvas coordinates. */
export const channelPort = (index: number, layouts: ChannelLayout[]) => ({
  x: source.x + channelWidth(layouts[index].expansion),
  y: channelTop(index, layouts) + source.portOffset,
});

/** Ports are spread evenly but never further apart than `portSpacing`. */
const portStep = (count: number): number =>
  Math.min(bus.portSpacing, bus.portSpan / Math.max(count, 1));

/**
 * How tall a bus node is drawn.
 *
 * The middle column flows the way the source column does: the node that opens
 * pushes the ones below it down rather than overlapping them.
 */
export const busHeight = (expanded: boolean): number =>
  expanded ? bus.heightExpanded : bus.height;

/** Top edge of bus node `index`, given which nodes above it are open. */
export const busTop = (index: number, expansions: boolean[]): number =>
  expansions
    .slice(0, index)
    .reduce<number>((top, expanded) => top + busHeight(expanded) + bus.gap, bus.top);

/** Total height of the bus column, or room for the empty note when there are none. */
export const busStackHeight = (expansions: boolean[]): number =>
  expansions.length === 0
    ? bus.emptyHeight
    : expansions.reduce<number>((total, expanded) => total + busHeight(expanded) + bus.gap, 0);

/**
 * A port on one bus node, in canvas coordinates.
 *
 * Members are listed on the node in the same order the ports run down its edge,
 * so a cable lands beside the tile naming what it carries.
 */
export const busPort = (
  busIndex: number,
  expansions: boolean[],
  portIndex: number,
  portCount: number,
  side: 'in' | 'out'
) => ({
  x: side === 'in' ? bus.x : bus.x + bus.width,
  y: busTop(busIndex, expansions) + bus.portOffset + portIndex * portStep(portCount),
});

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

export const tapeTop = (focus: DestinationFocus): number =>
  destination.top + castHeight(focus) + destination.gap;

export const outputTop = (index: number, focus: DestinationFocus): number =>
  tapeTop(focus) + tapeHeight(focus) + destination.gap + index * destination.outputStep;

/** Top edge of the first extra destination, below the hardware outputs. */
export const extraTop = (outputCount: number, focus: DestinationFocus): number =>
  outputTop(outputCount, focus) + destination.extraOffset;

/**
 * A cable between two ports: horizontal at both ends, curving in the middle, so
 * it leaves and lands square against the node it connects to.
 */
export const cablePath = (from: { x: number; y: number }, to: { x: number; y: number }): string =>
  `M${from.x},${from.y} C${from.x + patch.cableControlReach},${from.y} ` +
  `${to.x - patch.cableControlReach},${to.y} ${to.x},${to.y}`;

/** Overall canvas height — tall enough for whichever column runs longest. */
export const canvasHeight = (
  layouts: ChannelLayout[],
  focus: DestinationFocus,
  outputCount: number,
  extraCount: number,
  pickerOpen: boolean,
  busExpansions: boolean[]
): number => {
  const sourceColumn = source.top + sourceStackHeight(layouts) + canvas.bottomPadding;
  const destinationColumn =
    extraTop(outputCount, focus) +
    extraCount * destination.extraStep +
    (pickerOpen ? destination.pickerHeight : destination.addHeight) +
    28;
  const busColumn = bus.top + busStackHeight(busExpansions) + 40;

  return Math.max(canvas.minHeight, sourceColumn, destinationColumn, busColumn);
};
