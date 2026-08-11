import { Box, Group, Stack, Text } from '@mantine/core';
import { useDisclosure, useHover } from '@mantine/hooks';

import { border, color } from '../../../theme/tokens';
import type { AudioDeviceInfo } from '../../../types';
import { asGain } from '../format';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { DashedTarget } from '../primitives/DashedTarget';
import { DragBar } from '../primitives/DragBar';
import { Panel } from '../primitives/Panel';
import { Pill } from '../primitives/Pill';
import { StatusDot } from '../primitives/StatusDot';


type PickerRowProps = {
  device: AudioDeviceInfo;
  onPick: (deviceId: string) => void;
};

const PickerRow = ({ device, onPick }: PickerRowProps) => {
  const { hovered, ref } = useHover();

  return (
    <Group
      ref={ref}
      onClick={() => onPick(device.id)}
      gap="md"
      wrap="nowrap"
      px="lg"
      py="md"
      style={{
        borderBottom: border(),
        cursor: 'pointer',
        background: hovered ? color.panelHi : undefined,
      }}
    >
      <StatusDot tone="inert" />
      <Text size="sm" truncate style={{ flex: 1 }}>
        {device.name}
      </Text>
    </Group>
  );
};

/** Destinations for the master sum, with their role and trim. */
export const OutputsPanel = () => {
  const { outputs, available, selectOutput, cycleOutputRole, setOutputGain } = usePatchOutputs();
  const [picking, { open, close }] = useDisclosure(false);

  return (
    <Panel title="OUTPUTS" p="3xl" gap="xl">
      {outputs.map((output) => (
        <Stack
          key={output.id}
          gap="md"
          p="lg"
          style={{
            border: border(),
            borderRadius: 'var(--mantine-radius-lg)',
            opacity: output.live ? 1 : 0.5,
          }}
        >
          <Group gap="md" wrap="nowrap">
            <StatusDot
              size={8}
              tone={output.live ? 'accent' : 'inert'}
              onClick={() => selectOutput(output.id)}
            />
            <Text size="md" fw={500} truncate style={{ flex: 1, minWidth: 0 }}>
              {output.name}
            </Text>
            <Pill
              tone={output.role === 'MAIN' ? 'accent' : output.role === 'CUE' ? 'warn' : 'neutral'}
              onClick={() => cycleOutputRole(output.id)}
            >
              {output.role}
            </Pill>
          </Group>

          <Group gap="md" wrap="nowrap">
            <DragBar
              value={output.gainDb}
              min={-60}
              max={12}
              height={5}
              knob={[10, 16]}
              tone={output.live ? 'accent' : 'muted'}
              onChange={(value) => setOutputGain(output.id, value)}
            />
            <Text size="2xs" w={44} ta="right" c={color.textDim} style={{ flex: 'none' }}>
              {asGain(output.gainDb)}
            </Text>
          </Group>
        </Stack>
      ))}

      {picking && (
        <Box
          style={{
            border: border('acc'),
            borderRadius: 'var(--mantine-radius-lg)',
            overflow: 'hidden',
          }}
        >
          {available.length === 0 ? (
            <Text size="xs" c={color.textFaint} ta="center" p="lg">
              Every output device is already in the mix.
            </Text>
          ) : (
            available.map((device) => (
              <PickerRow
                key={device.id}
                device={device}
                onPick={(deviceId) => {
                  selectOutput(deviceId);
                  close();
                }}
              />
            ))
          )}
        </Box>
      )}

      <DashedTarget label="+ ADD AN OUTPUT" onClick={picking ? close : open} height={44} />
    </Panel>
  );
};
