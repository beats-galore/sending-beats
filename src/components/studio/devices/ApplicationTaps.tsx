import { Box, Group, Stack, Text } from '@mantine/core';

import { useApplicationAudio } from '../../../hooks';
import { border, color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';
import { StatusDot } from '../primitives/StatusDot';

/** Capture audio straight out of a running application. */
export const ApplicationTaps = () => {
  const { availableApps, activeCaptures, isLoading, actions } = useApplicationAudio();

  const isCapturing = (pid: number) => activeCaptures.some((capture) => capture.pid === pid);

  return (
    <Panel
      title="APPLICATION TAPS"
      p="3xl"
      action={
        <ActionButton
          tone="ghost"
          padding="0"
          size="xs"
          onClick={() => void actions.refreshApplications()}
        >
          {isLoading ? 'SCANNING…' : 'RESCAN'}
        </ActionButton>
      }
    >
      <Text size="xs" c={color.textDim} lh="lg">
        Capture audio straight out of a running app — no aggregate device, no loopback routing.
      </Text>

      <Stack gap={0}>
        {availableApps.length === 0 ? (
          <Text size="xs" c={color.textFaint} ta="center" py="3xl">
            No capturable applications found.
          </Text>
        ) : (
          availableApps.map((app) => {
            const capturing = isCapturing(app.pid);
            return (
              <Group key={app.pid} gap="xl" wrap="nowrap" py="lg" style={{ borderTop: border() }}>
                <StatusDot
                  size={8}
                  tone={capturing ? 'accent' : app.is_playing_audio ? 'warn' : 'inert'}
                />
                <Box style={{ flex: 1, minWidth: 0 }}>
                  <Text size="md" fw={500} truncate>
                    {app.name}
                  </Text>
                  <Text size="2xs" c={color.textFaintest} mt="3xs" truncate>
                    pid {app.pid}
                    {app.bundle_id ? ` · ${app.bundle_id}` : ''}
                  </Text>
                </Box>
                <Text size="2xs" c={color.textFaint} w={84} style={{ flex: 'none' }}>
                  {app.is_playing_audio ? 'playing' : 'idle'}
                </Text>
                <ActionButton
                  tone={capturing ? 'accent' : 'ghost'}
                  padding="7px 14px"
                  onClick={() =>
                    void (capturing
                      ? actions.stopCapturing(app.pid)
                      : actions.startCapturing(app.pid))
                  }
                >
                  {capturing ? 'CAPTURING' : 'CAPTURE'}
                </ActionButton>
              </Group>
            );
          })
        )}
      </Stack>
    </Panel>
  );
};
