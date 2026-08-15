import { Box, Group, Stack, Text } from '@mantine/core';

import { busTargetKey } from '../../../services/patch-color-service';
import { useStudioStore } from '../../../stores/studio-store';
import { useBusLevels } from '../../../stores/vu-meter-store';
import { layout } from '../../../theme/layout';
import { border, color, glow } from '../../../theme/tokens';
import type { Bus } from '../../../types/bus.types';
import { asGain, meterPosition } from '../format';
import { useNodeDrag, useNodeFront } from '../hooks/use-node-drag';
import { useNodeResize, useNodeRung, useUnshrink } from '../hooks/use-node-resize';
import { DragBar } from '../primitives/DragBar';
import { ExpandToggle } from '../primitives/ExpandToggle';
import { LevelMeter } from '../primitives/LevelMeter';
import { ResizeGrip } from '../primitives/ResizeGrip';
import { SectionLabel } from '../primitives/SectionLabel';
import { BusMemberTiles } from './BusMemberTiles';
import { BusMetering } from './BusMetering';
import { BusPorts } from './BusPorts';
import { busExpansionFor, busSize, NodeExpansion, nextExpansion } from './patch-geometry';
import type { NodeRect } from './patch-layout';

const GAIN_MIN = -60;
const GAIN_MAX = 12;

type BusNodeProps = {
  bus: Bus;
  /** Box in canvas coordinates, with anything the user arranged applied. */
  rect: NodeRect;
  /** Holds the ring. Does not open the node. */
  selected: boolean;
  onGainChange: (busId: string, gainDb: number) => void;
};

/**
 * One mix: what feeds it, what takes it, and the trim between.
 *
 * There is no separate master sum any more. A mix exists because something is
 * listening to it — a bus with no output is dropped rather than summed — so
 * every node here is a mix that is actually being produced.
 */
export const BusNode = ({ bus, rect, selected, onGainChange }: BusNodeProps) => {
  const levels = useBusLevels(bus.id);
  const select = useStudioStore((state) => state.select);

  const targetKey = busTargetKey(bus.id);
  const grab = useNodeDrag(targetKey, rect);
  const resize = useNodeResize(targetKey, rect, busSize('compact'));
  const setRung = useNodeRung(targetKey);
  const { front, bringToFront } = useNodeFront(targetKey);

  const expansion = busExpansionFor(rect);
  const compact = expansion === 'compact';
  const next = nextExpansion(expansion, NodeExpansion);
  const unshrink = useUnshrink(targetKey, compact);

  // A bus nobody sends to still produces silence for its outputs, which is not
  // the same as one carrying audio, and the node should not claim otherwise.
  const carrying = bus.inputs.length > 0;
  const gainDb = gainToDb(bus.gain);

  return (
    <Box
      // Held inside the node: the canvas clears the selection on any click that
      // reaches it, and a click on a node is not a click on the canvas.
      onClick={(event) => {
        event.stopPropagation();
        select({ kind: 'bus', busId: bus.id });
        unshrink();
      }}
      onPointerDown={bringToFront}
      style={{
        position: 'absolute',
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        background: color.bgRaised,
        border: border(selected ? 'acc' : carrying ? 'lineStrong' : 'line'),
        borderRadius: 'var(--mantine-radius-2xl)',
        boxShadow: 'var(--mantine-shadow-lg)',
        cursor: 'pointer',
        display: 'flex',
        flexDirection: 'column',
        zIndex: front ? 20 : undefined,
      }}
    >
      <Group
        h={layout.bus.headerHeight}
        px="lg"
        gap="sm"
        wrap="nowrap"
        onPointerDown={grab}
        style={{
          flex: 'none',
          borderBottom: border(),
          background: color.panelHi,
          borderRadius: 'var(--mantine-radius-xl) var(--mantine-radius-xl) 0 0',
          cursor: 'grab',
          // The title is the grip. Selecting it instead of dragging by it is
          // never what was meant.
          userSelect: 'none',
        }}
      >
        <Box
          style={{
            width: 7,
            height: 7,
            flex: 'none',
            borderRadius: '50%',
            background: carrying ? color.acc : color.textFaintest,
            boxShadow: carrying ? glow('acc') : undefined,
          }}
        />
        <Text
          ff="var(--mantine-font-family-headings)"
          fw={700}
          fz="xl"
          truncate
          style={{ flex: 1, minWidth: 0, letterSpacing: layout.tracking.wider }}
        >
          {bus.name}
        </Text>
        <Text size="2xs" c={color.textFaint} style={{ flex: 'none' }}>
          {bus.inputs.length} IN · {bus.outputs.length} OUT
        </Text>
        <ExpandToggle
          grows={NodeExpansion.indexOf(next) > NodeExpansion.indexOf(expansion)}
          onToggle={() => setRung(next)}
        />
      </Group>

      <Stack gap="sm" p="lg" style={{ flex: 1, minHeight: 0 }}>
        {/* Shrunk to nothing else, a mix is still its levels: whether audio is
            flowing is the one thing you cannot wait to open it to read. */}
        <Stack gap="3xs">
          <LevelMeter
            level={meterPosition(levels.left.peak_level)}
            height={compact ? layout.compactMeterHeight : undefined}
            surface="bgRaised"
            dimmed={!carrying}
          />
          <LevelMeter
            level={meterPosition(levels.right.peak_level)}
            height={compact ? layout.compactMeterHeight : undefined}
            surface="bgRaised"
            dimmed={!carrying}
          />
        </Stack>

        {!compact && (
          <>
            <BusMemberTiles bus={bus} />

            <Group gap="md" wrap="nowrap">
              <SectionLabel tracking="tight">GAIN</SectionLabel>
              <DragBar
                value={gainDb}
                min={GAIN_MIN}
                max={GAIN_MAX}
                height={5}
                tone={carrying ? 'accent' : 'muted'}
                knob={[10, 16]}
                onChange={(value) => onGainChange(bus.id, Math.round(value * 10) / 10)}
              />
              <Text
                size="2xs"
                w={52}
                ta="right"
                c={carrying ? color.acc : color.textFaint}
                style={{ flex: 'none' }}
              >
                {asGain(gainDb)}
              </Text>
            </Group>
          </>
        )}

        {expansion === 'expanded' && <BusMetering levels={levels} gainDb={gainDb} />}
      </Stack>

      {/* The grip holds its press back from the node, so bringing the node
          forward has to be asked for here rather than left to bubble. */}
      <ResizeGrip
        onResize={(event) => {
          bringToFront();
          resize(event);
        }}
      />

      <BusPorts bus={bus} carrying={carrying} />
    </Box>
  );
};

/**
 * Bus gain is stored as a linear multiplier, while every gain control on the
 * canvas works in dB, so the two have to be converted between rather than
 * shown in the engine's own units.
 */
const gainToDb = (gain: number): number => (gain > 0 ? 20 * Math.log10(gain) : GAIN_MIN);
