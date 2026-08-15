import { Box, Group, Stack, Text } from '@mantine/core';

import type { ReactNode } from 'react';
import { useStudioStore } from '../../../stores/studio-store';
import { useBusLevels } from '../../../stores/vu-meter-store';
import { layout } from '../../../theme/layout';
import { border, color, glow } from '../../../theme/tokens';
import type { Bus } from '../../../types/bus.types';
import { asGain, meterPosition } from '../format';
import { DragBar } from '../primitives/DragBar';
import { LevelColumn } from '../primitives/LevelColumn';
import { LevelMeter } from '../primitives/LevelMeter';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatRow } from '../primitives/StatRow';
import { BusMemberTiles } from './BusMemberTiles';
import { busHeight } from './patch-geometry';

const SCALE_MARKS = ['0', '-6', '-12', '-18', '-24', '-36', '-60'];
const GAIN_MIN = -60;
const GAIN_MAX = 12;

type BusNodeProps = {
  bus: Bus;
  top: number;
  /** Open, showing the metering column, the large readout and the stats. */
  expanded: boolean;
  onGainChange: (busId: string, gainDb: number) => void;
  /** Ports, which sit outside the node's own bounds. */
  ports: ReactNode;
};

/**
 * One mix: what feeds it, what takes it, and the trim between.
 *
 * There is no separate master sum any more. A mix exists because something is
 * listening to it — a bus with no output is dropped rather than summed — so
 * every node here is a mix that is actually being produced.
 */
export const BusNode = ({ bus, top, expanded, onGainChange, ports }: BusNodeProps) => {
  const levels = useBusLevels(bus.id);
  const select = useStudioStore((state) => state.select);

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
      }}
      style={{
        position: 'absolute',
        left: layout.bus.x,
        top,
        width: layout.bus.width,
        height: busHeight(expanded),
        background: color.bgRaised,
        border: border(expanded ? 'acc' : carrying ? 'lineStrong' : 'line'),
        borderRadius: 'var(--mantine-radius-2xl)',
        boxShadow: 'var(--mantine-shadow-lg)',
        cursor: 'pointer',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <Group
        h={layout.bus.headerHeight}
        px="lg"
        gap="sm"
        wrap="nowrap"
        style={{
          flex: 'none',
          borderBottom: border(),
          background: color.panelHi,
          borderRadius: 'var(--mantine-radius-xl) var(--mantine-radius-xl) 0 0',
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
      </Group>

      <Stack gap="sm" p="lg" style={{ flex: 1, minHeight: 0 }}>
        {/* Shut, the node still has to say how loud the mix is and where it
            goes — the same job the collapsed source card does. */}
        <Stack gap="3xs">
          <LevelMeter level={meterPosition(levels.left.peak_level)} dimmed={!carrying} />
          <LevelMeter level={meterPosition(levels.right.peak_level)} dimmed={!carrying} />
        </Stack>

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

        {expanded && (
          <Group gap="3xl" pt="md" align="stretch" wrap="nowrap" style={{ flex: 1, minHeight: 0 }}>
            <Group gap="lg" align="stretch" wrap="nowrap">
              <LevelColumn level={meterPosition(levels.left.peak_level)} />
              <LevelColumn level={meterPosition(levels.right.peak_level)} />
              <Stack justify="space-between" gap={0} py="3xs">
                {SCALE_MARKS.map((mark) => (
                  <Text key={mark} size="3xs" c={color.textFaintest}>
                    {mark}
                  </Text>
                ))}
              </Stack>
            </Group>

            <Stack gap="lg" style={{ flex: 1, minWidth: 0 }}>
              <Box
                p="md"
                style={{
                  background: color.bg,
                  border: border(),
                  borderRadius: 'var(--mantine-radius-md)',
                }}
              >
                <Text
                  fz="4xl"
                  fw={600}
                  c={color.acc}
                  style={{ letterSpacing: layout.tracking.tight }}
                >
                  {asGain(gainDb).replace('dB', '')}
                  <Text span fz="lg" c={color.textFaint}>
                    {' '}
                    dB
                  </Text>
                </Text>
                <SectionLabel tone="faint" tracking="widest" mt="3xs">
                  MIX GAIN
                </SectionLabel>
              </Box>

              <Stack gap="xs">
                <StatRow label="PEAK L/R">
                  <Text size="xs" c={color.textDim}>
                    {levels.left.peak_level.toFixed(2)} / {levels.right.peak_level.toFixed(2)}
                  </Text>
                </StatRow>
                <StatRow label="RMS L/R">
                  <Text size="xs" c={color.textDim}>
                    {levels.left.rms_level.toFixed(2)} / {levels.right.rms_level.toFixed(2)}
                  </Text>
                </StatRow>
                {/* Loudness metering is not produced by the engine yet. */}
                <StatRow label="LUFS-S">—</StatRow>
              </Stack>
            </Stack>
          </Group>
        )}
      </Stack>

      {ports}
    </Box>
  );
};

/**
 * Bus gain is stored as a linear multiplier, while every gain control on the
 * canvas works in dB, so the two have to be converted between rather than
 * shown in the engine's own units.
 */
const gainToDb = (gain: number): number => (gain > 0 ? 20 * Math.log10(gain) : GAIN_MIN);
