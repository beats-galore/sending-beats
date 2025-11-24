import { Card, Button, Stack, Text, TextInput, Textarea, Alert, Modal } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { IconPlus, IconAlertCircle } from '@tabler/icons-react';
import { useEffect, useState } from 'react';

import { useConfigurationStore } from '../stores/mixer-store';

type SaveAsNewConfigurationProps = {
  onConfigurationCreated?: () => void;
};

export const SaveAsNewConfiguration = ({ onConfigurationCreated }: SaveAsNewConfigurationProps) => {
  const [opened, { open, close }] = useDisclosure(false);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');

  const {
    activeSession,
    isLoading,
    error,
    loadConfigurations,
    saveSessionAsNewReusable,
    clearError,
  } = useConfigurationStore();

  // Load configurations on mount
  useEffect(() => {
    void loadConfigurations();
  }, [loadConfigurations]);

  // Pre-fill form when session is available
  useEffect(() => {
    if (activeSession && opened) {
      setName(activeSession?.configuration.name.replace(' (Session)', '') ?? '');
      setDescription(activeSession?.configuration.description ?? '');
    }
  }, [activeSession, opened]);

  const handleSave = async () => {
    if (!name.trim()) {
      return;
    }

    try {
      await saveSessionAsNewReusable(name.trim(), description.trim() || undefined);

      // Reset form and close modal
      setName('');
      setDescription('');
      close();
      onConfigurationCreated?.();
    } catch (err) {
      // Error is handled by the store
    }
  };

  const handleCancel = () => {
    setName('');
    setDescription('');
    clearError();
    close();
  };

  if (!activeSession) {
    return (
      <Card withBorder p="md">
        <Text size="sm" c="dimmed" ta="center">
          No active session found. Start a session to save it as a reusable configuration.
        </Text>
      </Card>
    );
  }

  return (
    <>
      <Card withBorder p="md">
        <Stack gap="sm">
          <Button
            leftSection={<IconPlus size={16} />}
            onClick={open}
            variant="filled"
            color="green"
            fullWidth
          >
            Save as New Configuration
          </Button>

          <Text size="xs" c="dimmed" ta="center">
            Create a new reusable configuration from the current session
          </Text>
        </Stack>
      </Card>

      <Modal
        opened={opened}
        onClose={handleCancel}
        title="Save as New Configuration"
        centered
        size="md"
      >
        <Stack gap="md">
          {/* Current Session Info */}
          <Card withBorder p="sm" bg="blue.0">
            <Text size="sm" fw={500} c="blue.8">
              Saving Current Session: {activeSession.configuration.name}
            </Text>
            {activeSession.configuration.description && (
              <Text size="xs" c="blue.7" mt="xs">
                {activeSession.configuration.description}
              </Text>
            )}
          </Card>

          {/* Form Fields */}
          <TextInput
            label="Configuration Name"
            placeholder="Enter configuration name"
            value={name}
            onChange={(event) => setName(event.currentTarget.value)}
            required
            maxLength={100}
            error={!name.trim() && 'Name is required'}
          />

          <Textarea
            label="Description"
            placeholder="Optional description"
            value={description}
            onChange={(event) => setDescription(event.currentTarget.value)}
            maxLength={500}
            autosize
            minRows={3}
            maxRows={5}
          />

          {description && (
            <Text size="xs" c="dimmed" ta="right">
              {description.length}/500 characters
            </Text>
          )}

          {/* Error Display */}
          {error && (
            <Alert icon={<IconAlertCircle size={16} />} color="red" variant="light">
              {error}
            </Alert>
          )}

          {/* Action Buttons */}
          <Stack gap="xs">
            <Button
              onClick={handleSave}
              disabled={!name.trim() || isLoading}
              loading={isLoading}
              color="green"
              fullWidth
            >
              Save Configuration
            </Button>

            <Button onClick={handleCancel} variant="light" disabled={isLoading} fullWidth>
              Cancel
            </Button>
          </Stack>
        </Stack>
      </Modal>
    </>
  );
};
