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

/** Input port `index` on the master bus, in canvas coordinates. */
export const busInPort = (index: number, count: number) => ({
  x: bus.x,
  y: bus.top + bus.portOffset + index * portStep(count),
});

/** Output port `index` on the master bus, in canvas coordinates. */
export const busOutPort = (index: number, count: number) => ({
  x: bus.x + bus.width,
  y: bus.top + bus.outPortOffset + index * portStep(count),
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
  pickerOpen: boolean
): number => {
  const sourceColumn = source.top + sourceStackHeight(layouts) + canvas.bottomPadding;
  const destinationColumn =
    extraTop(outputCount, focus) +
    extraCount * destination.extraStep +
    (pickerOpen ? destination.pickerHeight : destination.addHeight) +
    28;
  const busColumn = bus.top + bus.height + 40;

  return Math.max(canvas.minHeight, sourceColumn, destinationColumn, busColumn);
};
