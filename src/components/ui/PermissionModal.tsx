import { Modal, Text, Button, Stack, Alert, Code, List } from '@mantine/core';
import { IconAlertCircle, IconMicrophone } from '@tabler/icons-react';

type PermissionModalProps = {
  isOpen: boolean;
  onClose: () => void;
  onOpenSystemPreferences: () => void;
  isLoading?: boolean;
}

export const PermissionModal = ({
  isOpen,
  onClose,
  onOpenSystemPreferences,
  isLoading = false,
}: PermissionModalProps) => {
  return (
    <Modal
      opened={isOpen}
      onClose={onClose}
      title="Audio Capture Permission Required"
      size="lg"
      centered
      closeOnClickOutside={false}
      closeOnEscape={false}
    >
      <Stack gap="md">
        <Alert icon={<IconAlertCircle size={20} />} color="blue" variant="light">
          <Text size="sm">
            To capture audio from applications like Spotify, Music, and other audio apps, you need
            to grant microphone permissions.
          </Text>
        </Alert>

        <Text size="sm">
          This allows the app to use Core Audio Taps to capture audio from specific applications
          without affecting system audio or recording your microphone.
        </Text>

        <Text fw={500} size="sm">
          Please follow these steps:
        </Text>

        <List size="sm" spacing="xs" icon={<Text c="blue">•</Text>}>
          <List.Item>
            Open <Code>System Preferences → Security & Privacy → Privacy</Code>
          </List.Item>
          <List.Item>
            Select <Code>Microphone</Code> from the left sidebar
          </List.Item>
          <List.Item>
            Find <Code>SendinBeats</Code> in the list and check the box
          </List.Item>
          <List.Item>Return to the app and try selecting an application source again</List.Item>
        </List>

        <Alert icon={<IconMicrophone size={16} />} color="green" variant="light">
          <Text size="xs">
            <strong>Privacy Note:</strong> This permission only allows capturing audio from other
            applications. Your microphone and personal audio will not be recorded unless explicitly
            selected.
          </Text>
        </Alert>

        <Stack gap="xs" mt="md">
          <Button onClick={onOpenSystemPreferences} loading={isLoading} fullWidth>
            Open System Preferences
          </Button>

          <Button variant="light" onClick={onClose} fullWidth>
            I'll Do This Later
          </Button>
        </Stack>
      </Stack>
    </Modal>
  );
};
