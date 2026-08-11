import { Box, Group, Stack, Text } from '@mantine/core';

import { useApplicationAudio, useAudioPermissions } from '../../../hooks';
import { border, color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';
import { StatusDot } from '../primitives/StatusDot';

/** macOS permissions the audio pipeline depends on. */
export const PermissionsPanel = () => {
  const audio = useAudioPermissions();
  const applicationAudio = useApplicationAudio();

  const permissions = [
    {
      name: 'Microphone',
      note: 'Needed for hardware inputs',
      granted: audio.hasPermission,
      request: audio.requestPermissions,
    },
    {
      name: 'Screen & System Audio',
      note: 'Needed to tap running applications',
      granted: applicationAudio.permissionsGranted,
      request: applicationAudio.actions.requestPermissions,
    },
  ];

  return (
    <Panel title="PERMISSIONS" p="3xl" gap="xl">
      <Stack gap={0}>
        {permissions.map((permission) => (
          <Group
            key={permission.name}
            gap="lg"
            wrap="nowrap"
            py="md"
            style={{ borderTop: border() }}
          >
            <StatusDot
              size={8}
              tone={permission.granted ? 'accent' : permission.granted === null ? 'inert' : 'warn'}
            />
            <Box style={{ flex: 1, minWidth: 0 }}>
              <Text size="md">{permission.name}</Text>
              <Text size="2xs" c={color.textFaintest} mt="3xs">
                {permission.note}
              </Text>
            </Box>
            {permission.granted ? (
              <Text size="2xs" c={color.acc}>
                GRANTED
              </Text>
            ) : (
              <ActionButton
                tone="ghost"
                padding="5px 10px"
                onClick={() => void permission.request()}
              >
                GRANT
              </ActionButton>
            )}
          </Group>
        ))}
      </Stack>

      {audio.permissionInstructions && (
        <Text size="2xs" c={color.textFaint} lh="lg">
          {audio.permissionInstructions}
        </Text>
      )}
    </Panel>
  );
};
