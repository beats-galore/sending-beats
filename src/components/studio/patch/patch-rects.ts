// Every node's box on the canvas, with hand-placed nodes overriding the columns.
//
// Pure, like `patch-geometry`: given what is on the patch and what the user has
// arranged, it returns where each node goes. The canvas and the cables both read
// from here, so a dragged node and the wire leaving it can never disagree.
//
// A node dragged out of its column leaves its slot behind rather than closing
// it up. Nodes below would otherwise jump the moment a drag started, which is
// the one thing a direct-manipulation canvas must not do. The tidy action is
// what closes the gaps.
import type { PatchTargetKey } from '../../../services/patch-color-service';
import {
  busTargetKey,
  channelTargetKey,
  outputTargetKey,
  STREAM_TARGET_KEY,
  TAPE_TARGET_KEY,
} from '../../../services/patch-color-service';
import type { PatchPlacement } from '../../../services/patch-layout-service';
import { layout } from '../../../theme/layout';
import {
  busHeight,
  castHeight,
  channelHeight,
  channelWidth,
  stackTops,
  tapeHeight,
} from './patch-geometry';
import type { ChannelLayout, DestinationFocus } from './patch-geometry';
import { canvasHeightOf, resolveRect } from './patch-layout';
import type { NodeRect } from './patch-layout';

const { source, bus, destination } = layout;

export type Placements = Partial<Record<PatchTargetKey, PatchPlacement>>;

type PatchRectsInput = {
  channels: { id: number; layout: ChannelLayout }[];
  buses: { id: string; expanded: boolean }[];
  outputIds: string[];
  destinationFocus: DestinationFocus;
};

export type PatchRects = {
  /** By index, parallel to the channel list given. */
  channels: NodeRect[];
  buses: NodeRect[];
  outputs: NodeRect[];
  cast: NodeRect;
  tape: NodeRect;
  /** Top of the "patch a source" target, below the source column. */
  addSourceTop: number;
  /** Top of the "add a destination" target, below the destination column. */
  addDestinationTop: number;
  /** Tall enough for everything actually on the canvas. */
  height: number;
};

/** A stored height wins over the one the node's contents ask for. */
const resolveHeight = (
  placements: Placements,
  key: PatchTargetKey,
  computed: number
): number => placements[key]?.height ?? computed;

/** Where the next thing below a column goes, given where the column ended. */
const stackEnd = (tops: number[], heights: number[], top: number, gap: number): number =>
  tops.length === 0 ? top : tops[tops.length - 1] + heights[heights.length - 1] + gap;

export const resolvePatchRects = (
  { channels, buses, outputIds, destinationFocus }: PatchRectsInput,
  placements: Placements
): PatchRects => {
  const channelKeys = channels.map((channel) => channelTargetKey(channel.id));
  const channelHeights = channels.map((channel, index) =>
    resolveHeight(placements, channelKeys[index], channelHeight(channel.layout))
  );
  const channelTops = stackTops(channelHeights, source.top, source.gap);
  const channelRects = channels.map((channel, index) =>
    resolveRect(placements[channelKeys[index]], {
      left: source.x,
      top: channelTops[index],
      width: channelWidth(channel.layout.expansion),
      height: channelHeights[index],
    })
  );

  const busKeys = buses.map((entry) => busTargetKey(entry.id));
  const busHeights = buses.map((entry, index) =>
    resolveHeight(placements, busKeys[index], busHeight(entry.expanded))
  );
  const busTops = stackTops(busHeights, bus.top, bus.gap);
  const busRects = buses.map((entry, index) =>
    resolveRect(placements[busKeys[index]], {
      left: bus.x,
      top: busTops[index],
      width: bus.width,
      height: busHeights[index],
    })
  );

  // The right column runs cast, then tape, then the hardware outputs, each
  // group flowing from the one above it.
  const castSlotHeight = resolveHeight(placements, STREAM_TARGET_KEY, castHeight(destinationFocus));
  const castRect = resolveRect(placements[STREAM_TARGET_KEY], {
    left: destination.x,
    top: destination.top,
    width: destination.width,
    height: castSlotHeight,
  });

  const tapeSlotTop = destination.top + castSlotHeight + destination.gap;
  const tapeSlotHeight = resolveHeight(placements, TAPE_TARGET_KEY, tapeHeight(destinationFocus));
  const tapeRect = resolveRect(placements[TAPE_TARGET_KEY], {
    left: destination.x,
    top: tapeSlotTop,
    width: destination.width,
    height: tapeSlotHeight,
  });

  const outputKeys = outputIds.map(outputTargetKey);
  const outputHeights = outputKeys.map((key) =>
    resolveHeight(placements, key, destination.outputHeight)
  );
  const outputColumnTop = tapeSlotTop + tapeSlotHeight + destination.gap;
  const outputTops = stackTops(outputHeights, outputColumnTop, destination.outputGap);
  const outputRects = outputKeys.map((key, index) =>
    resolveRect(placements[key], {
      left: destination.x,
      top: outputTops[index],
      width: destination.width,
      height: outputHeights[index],
    })
  );

  const addSourceTop = stackEnd(channelTops, channelHeights, source.top, source.gap);
  const addDestinationTop =
    stackEnd(outputTops, outputHeights, outputColumnTop, destination.outputGap) +
    destination.extraOffset;

  const rects = [...channelRects, ...busRects, ...outputRects, castRect, tapeRect];

  return {
    channels: channelRects,
    buses: busRects,
    outputs: outputRects,
    cast: castRect,
    tape: tapeRect,
    addSourceTop,
    addDestinationTop,
    // The targets contribute their top rather than their bottom: the padding
    // below the last node is what is keeping room for them in the first place.
    height: canvasHeightOf([
      ...rects.map((rect) => rect.top + rect.height),
      addSourceTop,
      addDestinationTop,
    ]),
  };
};
