import { Box } from '@mantine/core';

import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { ActionButton } from '../primitives/ActionButton';

/**
 * Puts every node back in its column.
 *
 * Free placement means a patch can be shuffled into an unreadable state, and
 * the arrangement is stored rather than remade each session, so without this
 * there is no way home. Nothing is placed by hand until something is dragged,
 * so the button only appears once there is something to undo.
 *
 * Sits over the viewport rather than on the canvas so it keeps its size when
 * the canvas is scaled down to fit.
 */
export const PatchTidy = () => {
  const arranged = usePatchLayoutStore((state) => Object.keys(state.placements).length > 0);
  const tidy = usePatchLayoutStore((state) => state.tidy);

  if (!arranged) {
    return null;
  }

  return (
    <Box style={{ position: 'absolute', right: 16, top: 12, zIndex: 10 }}>
      <ActionButton onClick={() => void tidy()}>TIDY</ActionButton>
    </Box>
  );
};
