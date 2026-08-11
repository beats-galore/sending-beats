import { Box, Center, ScrollArea, Text } from '@mantine/core';
import { useElementSize } from '@mantine/hooks';

import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { OnAirDrawer } from '../shell/OnAirDrawer';
import { OnAirRail } from '../shell/OnAirRail';
import { PatchCanvas } from './PatchCanvas';

type PatchViewProps = {
  ready: boolean;
};

/**
 * The patchbay.
 *
 * The canvas is authored against a fixed coordinate space so cables can be drawn
 * between absolute positions; it is scaled down to whatever width is available
 * rather than reflowing, which would break the wiring.
 */
export const PatchView = ({ ready }: PatchViewProps) => {
  const drawerOpen = useStudioStore((state) => state.drawerOpen);
  const { ref: viewportRef, width: viewportWidth } = useElementSize();
  const { ref: canvasRef, height: canvasNaturalHeight } = useElementSize();

  const scale = viewportWidth > 0 ? Math.min(1, viewportWidth / layout.canvas.width) : 1;

  return (
    <Box style={{ display: 'flex', flex: 1, minHeight: 0, alignItems: 'stretch' }}>
      <Box ref={viewportRef} style={{ flex: 1, minWidth: 0 }}>
        {ready ? (
          <ScrollArea h="100%">
            <Box
              style={{
                width: '100%',
                height: canvasNaturalHeight * scale,
                overflow: 'hidden',
              }}
            >
              <Box
                ref={canvasRef}
                style={{ transform: `scale(${scale})`, transformOrigin: 'top left' }}
              >
                <PatchCanvas />
              </Box>
            </Box>
          </ScrollArea>
        ) : (
          <Center h="100%">
            <Text size="xs" c={color.textFaint}>
              Starting the engine…
            </Text>
          </Center>
        )}
      </Box>

      {drawerOpen ? <OnAirDrawer /> : <OnAirRail />}
    </Box>
  );
};
