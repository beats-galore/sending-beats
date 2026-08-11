import { Stack, Text } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { StatusDot } from '../primitives/StatusDot';

/** The collapsed edge of the on-air drawer — clicking it brings the panel back. */
export const OnAirRail = () => {
  const toggleDrawer = useStudioStore((state) => state.toggleDrawer);
  const { hovered, ref } = useHover();

  return (
    <Stack
      ref={ref}
      onClick={toggleDrawer}
      align="center"
      gap="xl"
      py="xl"
      w={layout.shell.drawerRailWidth}
      style={{
        flex: 'none',
        background: hovered ? color.panelHi : color.bgRaised,
        borderLeft: border(),
        cursor: 'pointer',
      }}
    >
      <Text size="md" c={color.textFaint}>
        ‹
      </Text>
      <StatusDot tone="warn" />
      <Text
        size="2xs"
        c={color.textFaint}
        style={{ letterSpacing: layout.tracking.section, writingMode: 'vertical-rl' }}
      >
        ON AIR · SESSION
      </Text>
    </Stack>
  );
};
