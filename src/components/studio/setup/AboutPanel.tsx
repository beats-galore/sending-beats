import { Box, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import { Panel } from '../primitives/Panel';

/** Build identity. */
export const AboutPanel = () => (
  <Panel title="ABOUT" p="3xl" gap="sm" style={{ flex: 1, minHeight: 0 }}>
    <Text size="sm" c={color.textDim} lh="xl">
      Sweet Beats Studio {__APP_VERSION__}
      <br />
      Tauri desktop build
    </Text>
    <Box style={{ flex: 1 }} />
    <Text size="2xs" c={color.textFaintest} lh="lg">
      Made for people broadcasting out of a back room.
    </Text>
  </Panel>
);
