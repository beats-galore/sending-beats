import { Group } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { EngineLine } from './EngineLine';
import { LiveChip } from './LiveChip';
import { PatchMenu } from './PatchMenu';
import { PatchSaveButton } from './PatchSaveButton';
import { StudioLogo } from './StudioLogo';
import { ViewTabs } from './ViewTabs';

/** Application chrome: identity, navigation, the loaded patch and the air state. */
export const TopBar = () => (
  <Group
    h={layout.shell.topBarHeight}
    px="4xl"
    gap="3xl"
    wrap="nowrap"
    style={{
      flex: 'none',
      background: color.bgRaised,
      borderBottom: border(),
    }}
  >
    <StudioLogo />
    <ViewTabs />

    <Group gap="xs" wrap="nowrap">
      <PatchMenu />
      <PatchSaveButton />
    </Group>

    <div style={{ flex: 1 }} />

    <LiveChip />
    <EngineLine />
  </Group>
);
