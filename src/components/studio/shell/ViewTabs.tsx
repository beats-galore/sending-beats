import { Box, Group } from '@mantine/core';

import { useStudioStore, StudioView } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';

const TAB_LABELS: Record<StudioView, string> = {
  patch: 'PATCH',
  tape: 'TAPE',
  cast: 'CAST',
  queues: 'QUEUES',
  devices: 'DEVICES',
  setup: 'SETUP',
};

/** The top-level destinations, as a single segmented control in the nav well. */
export const ViewTabs = () => {
  const view = useStudioStore((state) => state.view);
  const setView = useStudioStore((state) => state.setView);

  return (
    <Group
      gap="3xs"
      wrap="nowrap"
      p="3xs"
      style={{
        flex: 'none',
        background: color.panelNav,
        border: border(),
        borderRadius: 'var(--mantine-radius-md)',
      }}
    >
      {StudioView.map((tab) => {
        const active = view === tab;
        return (
          <Box
            key={tab}
            onClick={() => setView(tab)}
            fz="xs"
            style={{
              padding: '6px 14px',
              borderRadius: 'var(--mantine-radius-xs)',
              fontWeight: 600,
              letterSpacing: layout.tracking.wide,
              cursor: 'pointer',
              background: active ? color.acc : undefined,
              color: active ? color.bg : color.textDim,
            }}
          >
            {TAB_LABELS[tab]}
          </Box>
        );
      })}
    </Group>
  );
};
