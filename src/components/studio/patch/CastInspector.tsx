import { Stack } from '@mantine/core';

import { border } from '../../../theme/tokens';
import { TransmitterControls } from '../cast/TransmitterControls';

type CastInspectorProps = {
  isLive: boolean;
  isBusy: boolean;
  onToggle: () => void;
};

/** The transmitter controls as the cast node shows them, revealed inside the card. */
export const CastInspector = ({ isLive, isBusy, onToggle }: CastInspectorProps) => (
  <Stack
    gap="lg"
    pt="lg"
    style={{ borderTop: border() }}
    onClick={(event) => event.stopPropagation()}
  >
    <TransmitterControls density="compact" isLive={isLive} isBusy={isBusy} onToggle={onToggle} />
  </Stack>
);
