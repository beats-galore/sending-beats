import { Group, Stack, Text } from '@mantine/core';
import { useMemo } from 'react';

import { useAudioDevices } from '../../../hooks';
import { useConfigurationStore } from '../../../stores/mixer-store';
import { border, color } from '../../../theme/tokens';
import { Panel } from '../primitives/Panel';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatusDot } from '../primitives/StatusDot';

/** Every input the machine offers, and whether the patch is using it. */
export const HardwareInputs = () => {
  const { inputDevices } = useAudioDevices();
  const { activeSession } = useConfigurationStore();

  const patched = useMemo(
    () =>
      new Set<string>(
        activeSession?.configuredDevices
          .filter((device) => device.isInput)
          .map((device) => device.deviceIdentifier) ?? []
      ),
    [activeSession]
  );

  return (
    <Panel
      title="HARDWARE INPUTS"
      p="3xl"
      style={{ flex: 1, minHeight: 0 }}
      action={
        <SectionLabel tone="faint" tracking="tight">
          {inputDevices.length} found
        </SectionLabel>
      }
    >
      <Stack gap={0}>
        {inputDevices.map((device) => {
          const inUse = patched.has(device.id);
          return (
            <Group key={device.id} gap="xl" wrap="nowrap" py="lg" style={{ borderTop: border() }}>
              <StatusDot size={8} tone={inUse ? 'accent' : 'inert'} />
              <Text size="md" truncate style={{ flex: 1, minWidth: 0 }}>
                {device.name}
              </Text>
              <Text size="2xs" c={color.textFaintest} w={120} style={{ flex: 'none' }}>
                {device.supported_channels[0] ?? 2} ch ·{' '}
                {(device.supported_sample_rates[0] ?? 48000) / 1000} kHz
              </Text>
              <Text
                size="2xs"
                w={96}
                c={inUse ? color.acc : color.textFaintest}
                style={{ flex: 'none' }}
              >
                {inUse ? 'IN USE' : 'AVAILABLE'}
              </Text>
            </Group>
          );
        })}
      </Stack>
    </Panel>
  );
};
