import { Box, Group, Stack, Text } from '@mantine/core';

import type { ReactNode } from 'react';
import { useMasterLevels, useMasterSectionData } from '../../../hooks';
import { layout } from '../../../theme/layout';
import { border, color, glow } from '../../../theme/tokens';
import { asGain, meterPosition } from '../format';
import { DragBar } from '../primitives/DragBar';
import { LevelColumn } from '../primitives/LevelColumn';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatRow } from '../primitives/StatRow';

const SCALE_MARKS = ['0', '-6', '-12', '-18', '-24', '-36', '-60'];
const GAIN_MIN = -60;
const GAIN_MAX = 12;

type MasterBusProps = {
  inputCount: number;
  outputCount: number;
  inPorts: ReactNode;
  outPorts: ReactNode;
};

/** Where every source sums before it leaves for its destinations. */
export const MasterBus = ({ inputCount, outputCount, inPorts, outPorts }: MasterBusProps) => {
  const levels = useMasterLevels();
  const { mixerConfig, setMasterGain } = useMasterSectionData();
  const masterGain = mixerConfig?.master_gain ?? 0;

  return (
    <Box
      style={{
        position: 'absolute',
        left: layout.bus.x,
        top: layout.bus.top,
        width: layout.bus.width,
        height: layout.bus.height,
        background: color.bgRaised,
        border: border('lineStrong'),
        borderRadius: 'var(--mantine-radius-2xl)',
        boxShadow: 'var(--mantine-shadow-lg)',
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
            borderRadius: '50%',
            background: color.acc,
            boxShadow: glow('acc'),
          }}
        />
        <Text
          ff="var(--mantine-font-family-headings)"
          fw={700}
          fz="xl"
          style={{ flex: 1, letterSpacing: layout.tracking.wider }}
        >
          MASTER SUM
        </Text>
        <Text size="2xs" c={color.textFaint}>
          {inputCount} IN · {outputCount} OUT
        </Text>
      </Group>

      <Group gap="3xl" p="2xl" align="stretch" wrap="nowrap" style={{ flex: 1, minHeight: 0 }}>
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
            <Text fz="4xl" fw={600} c={color.acc} style={{ letterSpacing: layout.tracking.tight }}>
              {asGain(masterGain).replace('dB', '')}
              <Text span fz="lg" c={color.textFaint}>
                {' '}
                dB
              </Text>
            </Text>
            <SectionLabel tone="faint" tracking="widest" mt="3xs">
              MASTER GAIN
            </SectionLabel>
          </Box>

          <DragBar
            value={masterGain}
            min={GAIN_MIN}
            max={GAIN_MAX}
            onChange={(value) => setMasterGain(Math.round(value * 10) / 10)}
            height={8}
            knob={[14, 22]}
          />

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

      {inPorts}
      {outPorts}
    </Box>
  );
};
