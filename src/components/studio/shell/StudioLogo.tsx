import { Box, Group, Text } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

/** The app mark: a patch point in a frame, and the wordmark beside it. */
export const StudioLogo = () => (
  <Group gap="md" wrap="nowrap">
    <Box
      style={{
        width: 22,
        height: 22,
        flex: 'none',
        border: `2px solid ${color.acc}`,
        borderRadius: 'var(--mantine-radius-sm)',
        position: 'relative',
      }}
    >
      <Box
        style={{
          position: 'absolute',
          left: '50%',
          top: '50%',
          transform: 'translate(-50%, -50%)',
          width: 8,
          height: 8,
          background: color.acc,
          borderRadius: '50%',
        }}
      />
    </Box>
    <Text
      ff="var(--mantine-font-family-headings)"
      fw={700}
      fz="2xl"
      style={{ letterSpacing: layout.tracking.wide, whiteSpace: 'nowrap' }}
    >
      SENDIN BEATS STUDIO
    </Text>
  </Group>
);
