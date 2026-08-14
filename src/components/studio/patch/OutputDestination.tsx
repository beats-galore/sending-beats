import { Group, Text } from '@mantine/core';

import type { DestinationRole } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { asGain } from '../format';
import type { PatchOutput } from '../hooks/use-patch-outputs';
import { DragBar } from '../primitives/DragBar';
import { NodeCard } from '../primitives/NodeCard';
import { Pill } from '../primitives/Pill';
import { PortDot } from '../primitives/PortDot';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatusDot } from '../primitives/StatusDot';

const { destination } = layout;

const ROLE_PILL: Record<DestinationRole, 'accent' | 'warn' | 'neutral'> = {
  MAIN: 'accent',
  CUE: 'warn',
  SEND: 'neutral',
};

/** Cue sends are flagged amber on the patch point; everything else reads as signal. */
const ROLE_PORT: Record<DestinationRole, 'accent' | 'warn'> = {
  MAIN: 'accent',
  CUE: 'warn',
  SEND: 'accent',
};

const ROLE_COLOR: Record<DestinationRole, string> = {
  MAIN: color.acc,
  CUE: color.warn,
  SEND: color.textDim,
};

type OutputDestinationProps = {
  output: PatchOutput;
  top: number;
  onSelect: (deviceId: string) => void;
  onCycleRole: (deviceId: string) => void;
  onGainChange: (deviceId: string, gainDb: number) => void;
};

/** One hardware output the master sum can feed. */
export const OutputDestination = ({
  output,
  top,
  onSelect,
  onCycleRole,
  onGainChange,
}: OutputDestinationProps) => {
  // A destination the configuration still points at but which never connected.
  // Without this it renders as live and the mix appears to be going somewhere.
  const unavailable = output.unavailableReason !== null;

  return (
    <NodeCard
      position={{
        left: destination.x,
        top,
        width: destination.width,
        height: destination.outputHeight,
      }}
      borderColor={unavailable ? color.hotBorder : output.live ? color.line : color.dash}
      dimmed={!output.live}
      ports={
        <PortDot
          tone={unavailable ? 'dead' : output.live ? ROLE_PORT[output.role] : 'dead'}
          side="left"
          top={24}
        />
      }
      header={
        <>
          <StatusDot
            tone={unavailable ? 'hot' : output.live ? 'accent' : 'inert'}
            onClick={() => onSelect(output.id)}
            title={
              output.unavailableReason ??
              (output.live ? 'Receiving the master sum' : 'Send the master sum here')
            }
          />
          <Text
            ff="var(--mantine-font-family-headings)"
            fw={600}
            fz="md"
            truncate
            style={{ flex: 1, letterSpacing: layout.tracking.tight }}
          >
            {output.name}
          </Text>
          {unavailable ? (
            <Pill tone="hot" title={output.unavailableReason ?? undefined}>
              OFFLINE
            </Pill>
          ) : (
            <Pill tone={ROLE_PILL[output.role]} onClick={() => onCycleRole(output.id)}>
              {output.role}
            </Pill>
          )}
        </>
      }
      bodyStyle={{ padding: '0 11px', display: 'flex', alignItems: 'center' }}
    >
      <Group gap="md" wrap="nowrap" w="100%">
        <SectionLabel tracking="tight">GAIN</SectionLabel>
        <DragBar
          value={output.gainDb}
          min={-60}
          max={12}
          height={5}
          tone={output.live ? 'accent' : 'muted'}
          knob={[10, 16]}
          onChange={(value) => onGainChange(output.id, value)}
        />
        <Text
          size="2xs"
          w={52}
          ta="right"
          c={output.live ? ROLE_COLOR[output.role] : color.textFaint}
          style={{ flex: 'none' }}
        >
          {asGain(output.gainDb)}
        </Text>
      </Group>
    </NodeCard>
  );
};
