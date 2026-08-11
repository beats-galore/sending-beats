import { Group, Stack } from '@mantine/core';

import { AboutPanel } from './AboutPanel';
import { AudioEnginePanel } from './AudioEnginePanel';
import { LaunchPanel } from './LaunchPanel';
import { PermissionsPanel } from './PermissionsPanel';
import { ShortcutsPanel } from './ShortcutsPanel';

/** Engine state, permissions, shortcuts and preferences. */
export const SetupView = () => (
  <Group
    align="stretch"
    gap="4xl"
    p="5xl"
    wrap="nowrap"
    style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}
  >
    <Stack gap="2xl" style={{ flex: 1, minWidth: 0 }}>
      <AudioEnginePanel />
      <PermissionsPanel />
      <ShortcutsPanel />
    </Stack>

    <Stack w={380} gap="2xl" style={{ flex: 'none' }}>
      <LaunchPanel />
      <AboutPanel />
    </Stack>
  </Group>
);
