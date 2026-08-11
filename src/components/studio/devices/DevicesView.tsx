import { Group, Stack } from '@mantine/core';

import { ApplicationTaps } from './ApplicationTaps';
import { HardwareInputs } from './HardwareInputs';
import { OutputsPanel } from './OutputsPanel';
import { VirtualDevicePanel } from './VirtualDevicePanel';

/** Everything the machine can hear and everything it can speak through. */
export const DevicesView = () => (
  <Group
    align="stretch"
    gap="4xl"
    p="5xl"
    wrap="nowrap"
    style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}
  >
    <Stack gap="2xl" style={{ flex: 1, minWidth: 0 }}>
      <ApplicationTaps />
      <HardwareInputs />
    </Stack>

    <Stack w={380} gap="2xl" style={{ flex: 'none' }}>
      <OutputsPanel />
      <VirtualDevicePanel />
    </Stack>
  </Group>
);
