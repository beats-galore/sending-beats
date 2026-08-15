import { Group, NativeSelect, Text } from '@mantine/core';

import type { DestinationRole } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { asGain } from '../format';
import type { PatchOutput } from '../hooks/use-patch-outputs';
import { DeleteButton } from '../primitives/DeleteButton';
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
  options: { value: string; label: string }[];
  /** Why the last attempt to re-point this destination failed, if it did */
  switchError: string | null;
  onSelect: (deviceId: string) => void;
  onChangeDevice: (oldDeviceId: string, newDeviceId: string) => void;
  onCycleRole: (deviceId: string) => void;
  onGainChange: (deviceId: string, gainDb: number) => void;
  onRemove: (deviceId: string) => void;
};

/** One hardware output the master sum can feed. */
export const OutputDestination = ({
  output,
  top,
  options,
  switchError,
  onSelect,
  onChangeDevice,
  onCycleRole,
  onGainChange,
  onRemove,
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
          <NativeSelect
            value={output.id}
            onChange={(event) => onChangeDevice(output.id, event.currentTarget.value)}
            onClick={(event) => event.stopPropagation()}
            data={options}
            variant="unstyled"
            style={{ flex: 1, minWidth: 0 }}
            styles={{
              input: {
                fontFamily: 'var(--mantine-font-family-headings)',
                fontWeight: 600,
                fontSize: 'var(--mantine-font-size-md)',
                letterSpacing: layout.tracking.tight,
                color: unavailable ? color.hotText : color.text,
              },
            }}
          />
          {switchError ? (
            <Pill tone="hot" title={switchError}>
              FAILED
            </Pill>
          ) : unavailable ? (
            <Pill tone="hot" title={output.unavailableReason ?? undefined}>
              OFFLINE
            </Pill>
          ) : (
            <Pill tone={ROLE_PILL[output.role]} onClick={() => onCycleRole(output.id)}>
              {output.role}
            </Pill>
          )}
          <DeleteButton
            onDelete={() => onRemove(output.id)}
            title={`Remove ${output.name} from the mix`}
          />
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
