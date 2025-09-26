import { Card, Button, Stack, Badge, Text, Alert } from '@mantine/core';
import { IconCheck, IconAlertCircle, IconDeviceFloppy } from '@tabler/icons-react';
import { useEffect, useState } from 'react';

import { useConfigurationStore } from '../stores/mixer-store';

type ConfigurationSaverProps = {
  onConfigurationSaved?: () => void;
}

export const ConfigurationSaver = ({ onConfigurationSaved }: ConfigurationSaverProps) => {
  const {
    activeSession,
    isLoading,
    error,
    loadConfigurations,
    saveSessionToReusable,
    clearError,
  } = useConfigurationStore();

  // Load configurations on mount
  useEffect(() => {
    void loadConfigurations();
  }, [loadConfigurations]);

  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const handleSaveToReusable = async () => {
    try {
      await saveSessionToReusable();

      setSuccessMessage('Configuration saved successfully!');
      setTimeout(() => setSuccessMessage(null), 3000);

      onConfigurationSaved?.();
    } catch (err) {
      // Error is handled by the store
    }
  };

  const canSaveToReusable = activeSession?.configuration?.reusableConfigurationId != null;

  if (!activeSession) {
    return (
      <Card withBorder p="md">
        <Text size="sm" c="dimmed" ta="center">
          No active session found. Select a reusable configuration to start a session.
        </Text>
      </Card>
    );
  }

  return (
    <Card withBorder p="md">
      <Stack gap="sm">
        {/* Active Session Display */}
        <Badge
          leftSection={<IconCheck size={14} />}
          color="blue"
          variant="light"
          size="lg"
          fullWidth
        >
          Current: {activeSession.configuration.name}
        </Badge>

        {activeSession.configuration.description && (
          <Text size="xs" c="dimmed" ta="center">
            {activeSession.configuration.description}
          </Text>
        )}

        {/* Save Button */}
        <Button
          leftSection={<IconDeviceFloppy size={16} />}
          onClick={handleSaveToReusable}
          disabled={!canSaveToReusable || isLoading}
          loading={isLoading}
          variant="filled"
          color="blue"
          fullWidth
        >
          Save to Reusable Configuration
        </Button>

        {/* Status Messages */}
        {canSaveToReusable && (
          <Text size="xs" c="green" ta="center">
            ✓ Linked to reusable configuration - changes can be saved
          </Text>
        )}

        {!canSaveToReusable && (
          <Text size="xs" c="dimmed" ta="center">
            This session is not linked to a reusable configuration.
            Use "Save as New" to create a reusable configuration.
          </Text>
        )}

        {/* Success Message */}
        {successMessage && (
          <Alert
            icon={<IconCheck size={16} />}
            color="green"
            variant="light"
          >
            {successMessage}
          </Alert>
        )}

        {/* Error Display */}
        {error && (
          <Alert
            icon={<IconAlertCircle size={16} />}
            color="red"
            variant="light"
            withCloseButton
            onClose={clearError}
          >
            {error}
          </Alert>
        )}

        {/* Empty State for no session link */}
        {!canSaveToReusable && (
          <Text size="sm" c="orange" ta="center">
            Cannot save: Session not linked to reusable configuration
          </Text>
        )}
      </Stack>
    </Card>
  );
};