import {
  Card,
  Title,
  Stack,
  Table,
  Badge,
  Button,
  Group,
  Text,
  Alert,
  Loader,
  ActionIcon,
  Tooltip,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconCheck, IconPlus, IconTrash, IconRefresh, IconAlertCircle } from '@tabler/icons-react';
import { memo, useEffect, useState } from 'react';

import { useApplicationManager } from '../../hooks';

import type { AvailableApplicationInfo } from '../../types/applicationAudio.types';

const useStyles = createStyles((theme) => ({
  card: {
    backgroundColor: theme.colors.dark[7],
    border: `1px solid ${theme.colors.dark[4]}`,
  },
  table: {
    '& thead tr th': {
      backgroundColor: theme.colors.dark[6],
      color: theme.colors.gray[4],
      fontWeight: 600,
      borderBottom: `2px solid ${theme.colors.dark[4]}`,
    },
    '& tbody tr td': {
      borderBottom: `1px solid ${theme.colors.dark[5]}`,
    },
  },
}));

export const ApplicationAudioManager = memo(() => {
  const { classes } = useStyles();
  const {
    allApplications,
    isLoading,
    error,
    loadAllApplications,
    addApplication,
    removeApplication,
    clearError,
  } = useApplicationManager();

  const [actionInProgress, setActionInProgress] = useState<string | null>(null);

  useEffect(() => {
    loadAllApplications();
  }, [loadAllApplications]);

  const handleAddApplication = async (app: AvailableApplicationInfo) => {
    setActionInProgress(app.bundleIdentifier);
    try {
      await addApplication(app.bundleIdentifier, app.applicationName, 'macos');
    } catch (err) {
      console.error('Failed to add application:', err);
    } finally {
      setActionInProgress(null);
    }
  };

  const handleRemoveApplication = async (bundleIdentifier: string) => {
    setActionInProgress(bundleIdentifier);
    try {
      await removeApplication(bundleIdentifier);
    } catch (err) {
      console.error('Failed to remove application:', err);
    } finally {
      setActionInProgress(null);
    }
  };

  const handleRefresh = async () => {
    clearError();
    await loadAllApplications();
  };

  const rows = allApplications.map((app) => (
    <Table.Tr key={app.bundleIdentifier}>
      <Table.Td>
        <Text fw={500}>{app.applicationName}</Text>
      </Table.Td>
      <Table.Td>
        <Text size="sm" c="dimmed">
          {app.bundleIdentifier}
        </Text>
      </Table.Td>
      <Table.Td>
        {app.isInDatabase ? (
          <Badge color="green" variant="light" leftSection={<IconCheck size={14} />}>
            In Database
          </Badge>
        ) : (
          <Badge color="gray" variant="light">
            Not Added
          </Badge>
        )}
      </Table.Td>
      <Table.Td>
        <Group gap="xs" justify="flex-end">
          {app.isInDatabase ? (
            <Tooltip label="Remove from database">
              <ActionIcon
                color="red"
                variant="light"
                onClick={() => handleRemoveApplication(app.bundleIdentifier)}
                loading={actionInProgress === app.bundleIdentifier}
              >
                <IconTrash size={18} />
              </ActionIcon>
            </Tooltip>
          ) : (
            <Tooltip label="Add to database">
              <ActionIcon
                color="blue"
                variant="light"
                onClick={() => handleAddApplication(app)}
                loading={actionInProgress === app.bundleIdentifier}
              >
                <IconPlus size={18} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      </Table.Td>
    </Table.Tr>
  ));

  return (
    <Card className={classes.card} padding="lg" withBorder>
      <Stack gap="md">
        <Group justify="space-between">
          <Title order={3}>Application Audio Manager</Title>
          <Tooltip label="Refresh application list">
            <Button
              leftSection={<IconRefresh size={16} />}
              variant="light"
              onClick={handleRefresh}
              loading={isLoading}
            >
              Refresh
            </Button>
          </Tooltip>
        </Group>

        {error && (
          <Alert
            icon={<IconAlertCircle size={16} />}
            title="Error"
            color="red"
            withCloseButton
            onClose={clearError}
          >
            {error}
          </Alert>
        )}

        {isLoading && allApplications.length === 0 ? (
          <Group justify="center" py="xl">
            <Loader size="md" />
            <Text c="dimmed">Loading applications...</Text>
          </Group>
        ) : allApplications.length === 0 ? (
          <Alert icon={<IconAlertCircle size={16} />} color="blue">
            No applications found. Make sure you have granted Screen Recording permission.
          </Alert>
        ) : (
          <Table className={classes.table} striped highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Application Name</Table.Th>
                <Table.Th>Bundle Identifier</Table.Th>
                <Table.Th>Status</Table.Th>
                <Table.Th style={{ width: 120, textAlign: 'right' }}>Actions</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>{rows}</Table.Tbody>
          </Table>
        )}

        <Text size="sm" c="dimmed">
          Applications shown here are detected via ScreenCaptureKit. Add applications to enable
          audio capture from them.
        </Text>
      </Stack>
    </Card>
  );
});

ApplicationAudioManager.displayName = 'ApplicationAudioManager';
