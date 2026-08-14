import { Stack } from '@mantine/core';

import { border } from '../../../theme/tokens';
import type { useTapeTransport } from '../hooks/use-tape-transport';
import { TapeOutputControls } from '../tape/TapeOutputControls';

type TapeInspectorProps = {
  tape: ReturnType<typeof useTapeTransport>;
};

/** The output controls as the tape node shows them, revealed inside the card. */
export const TapeInspector = ({ tape }: TapeInspectorProps) => (
  <Stack
    gap="lg"
    pt="lg"
    style={{ borderTop: border() }}
    onClick={(event) => event.stopPropagation()}
  >
    <TapeOutputControls density="compact" tape={tape} />
  </Stack>
);
