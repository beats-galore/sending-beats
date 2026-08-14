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

export const channelWidth = (expansion: ChannelExpansion): number =>
  expansion === 'collapsed' ? source.width : source.widthExpanded;

export const channelHeight = (expansion: ChannelExpansion): number => {
  switch (expansion) {
    case 'collapsed':
      return source.height;
    case 'inspector':
      return source.heightInspector;
    case 'effects':
      return source.heightExpanded;
  }
};

/** Top edge of the node at `index`, given how far each node before it is opened. */
export const channelTop = (index: number, expansion: ChannelExpansion[]): number =>
  expansion
    .slice(0, index)
    .reduce<number>((top, state) => top + channelHeight(state) + source.gap, source.top);

/** Total height of the source column including the trailing gap. */
export const sourceStackHeight = (expansion: ChannelExpansion[]): number =>
  expansion.reduce<number>((total, state) => total + channelHeight(state) + source.gap, 0);

/** Where a channel's output port sits, in canvas coordinates. */
export const channelPort = (index: number, expansion: ChannelExpansion[]) => ({
  x: source.x + channelWidth(expansion[index]),
  y: channelTop(index, expansion) + source.portOffset,
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

export const outputTop = (index: number): number =>
  destination.outputTop + index * destination.outputStep;

/** Top edge of the first extra destination, below the hardware outputs. */
export const extraTop = (outputCount: number): number =>
  outputTop(outputCount) + destination.extraOffset;

/**
 * A cable between two ports: horizontal at both ends, curving in the middle, so
 * it leaves and lands square against the node it connects to.
 */
export const cablePath = (from: { x: number; y: number }, to: { x: number; y: number }): string =>
  `M${from.x},${from.y} C${from.x + patch.cableControlReach},${from.y} ` +
  `${to.x - patch.cableControlReach},${to.y} ${to.x},${to.y}`;

/** Overall canvas height — tall enough for whichever column runs longest. */
export const canvasHeight = (
  expansion: ChannelExpansion[],
  outputCount: number,
  extraCount: number,
  pickerOpen: boolean
): number => {
  const sourceColumn = source.top + sourceStackHeight(expansion) + canvas.bottomPadding;
  const destinationColumn =
    extraTop(outputCount) +
    extraCount * destination.extraStep +
    (pickerOpen ? destination.pickerHeight : destination.addHeight) +
    28;
  const busColumn = bus.top + bus.height + 40;

  return Math.max(canvas.minHeight, sourceColumn, destinationColumn, busColumn);
};
