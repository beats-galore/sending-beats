import { Group, NativeSelect, Text } from '@mantine/core';

import type { DestinationRole } from '../../../stores/studio-store';
import { color } from '../../../theme/tokens';
import { asGain } from '../format';
import type { PatchOutput } from '../hooks/use-patch-outputs';
import { DragBar } from '../primitives/DragBar';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatusDot } from '../primitives/StatusDot';
import { SourceTiles } from './SourceTiles';

const ROLE_COLOR: Record<DestinationRole, string> = {
  MAIN: color.acc,
  CUE: color.warn,
  SEND: color.textDim,
};

type OutputBodyProps = {
  output: PatchOutput;
  /** Shrunk to nothing but the tiles saying which mixes feed it. */
  compact: boolean;
  options: { value: string; label: string }[];
  onSelect: (deviceId: string) => void;
  onChangeDevice: (oldDeviceId: string, newDeviceId: string) => void;
  onGainChange: (deviceId: string, gainDb: number) => void;
};

/**
 * What a destination is patched to, how loud, and what feeds it.
 *
 * The routing outlives the rest: a destination shrunk as far as it goes keeps
 * its FROM tiles and gives up the picker and the trim, because which mixes
 * arrive here is the thing a destination is read for at a glance.
 */
export const OutputBody = ({
  output,
  compact,
  options,
  onSelect,
  onChangeDevice,
  onGainChange,
}: OutputBodyProps) => {
  const unavailable = output.unavailableReason !== null;

  return (
    <>
      {!compact && (
        <>
          <Group gap="xs" wrap="nowrap">
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
                  color: unavailable ? color.hotText : color.textDim,
                  fontSize: 'var(--mantine-font-size-xs)',
                },
              }}
            />
          </Group>

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
        </>
      )}

      <Group gap="md" wrap="nowrap" w="100%" style={compact ? { flex: 1 } : undefined}>
        <SectionLabel tracking="tight">FROM</SectionLabel>
        <SourceTiles deviceId={output.id} />
      </Group>
    </>
  );
};
