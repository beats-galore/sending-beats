import {
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
  Modal,
  ButtonGroup,
  Flex,
  ScrollArea,
  TextInput,
} from '@mantine/core';
import type { UseDisclosureHandlers } from '@mantine/hooks';
import { createStyles } from '@mantine/styles';
import {
  IconCheck,
  IconPlus,
  IconTrash,
  IconRefresh,
  IconAlertCircle,
  IconPencil,
} from '@tabler/icons-react';
import { memo, useCallback, useEffect, useMemo, useState } from 'react';

import { useApplicationAudioStore } from '../../stores/application-audio-store';

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
  clickableNameGroup: {
    cursor: 'pointer',
    padding: '4px 8px',
    margin: '-4px -8px',
    borderRadius: theme.radius.sm,
    '&:hover': {
      backgroundColor: theme.fn.rgba(theme.colors.blue[6], 0.1),
    },
  },
}));

export const ApplicationAudioManager = memo<
  Omit<UseDisclosureHandlers, 'toggle'> & { opened: boolean }
>(({ open: _open, close, opened }) => {
  const { classes } = useStyles();
  const allApplications = useApplicationAudioStore((state) => state.allApplications);
  const isLoading = useApplicationAudioStore((state) => state.isLoading);
  const error = useApplicationAudioStore((state) => state.error);
  const loadAllApplications = useApplicationAudioStore((state) => state.loadAllApplications);
  const addApplication = useApplicationAudioStore((state) => state.addApplication);
  const updateApplicationName = useApplicationAudioStore((state) => state.updateApplicationName);
  const removeApplication = useApplicationAudioStore((state) => state.removeApplication);
  const refreshApplications = useApplicationAudioStore((state) => state.refreshApplications);
  const clearError = useApplicationAudioStore((state) => state.clearError);

  const [originalApps, setOriginalApps] = useState<AvailableApplicationInfo[]>();
  const [updatedApps, setUpdatedApps] = useState<AvailableApplicationInfo[]>();
  const [originalSort, setOriginalSort] = useState<string[]>();

  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const [editingBundleId, setEditingBundleId] = useState<string | null>(null);
  const [editedName, setEditedName] = useState<string>('');
  const [nameChanges, setNameChanges] = useState<Record<string, string>>({});

  useEffect(() => {
    loadAllApplications();
  }, [loadAllApplications]);

  useEffect(() => {
    if (isLoading) {
      return;
    }
    setOriginalApps(allApplications);
    setUpdatedApps(allApplications.filter((a) => a.isInDatabase));
    setOriginalSort(
      allApplications
        .sort((a, b) => {
          if (a.isInDatabase && !b.isInDatabase) {
            return -1;
          }
          if (b.isInDatabase && !a.isInDatabase) {
            return 1;
          }
          return a.applicationName < b.applicationName ? -1 : 1;
        })
        .map((a) => a.bundleIdentifier)
    );
  }, [isLoading, allApplications, setOriginalApps, setUpdatedApps]);

  const handleSaveApplication = useCallback(
    async (app: AvailableApplicationInfo, customName?: string) => {
      setActionInProgress(app.bundleIdentifier);
      try {
        const nameToUse = customName ?? app.applicationName;
        await addApplication(app.bundleIdentifier, nameToUse, 'macos');
      } catch (err) {
        console.error('Failed to add application:', err);
      } finally {
        setActionInProgress(null);
      }
    },
    [setActionInProgress, addApplication]
  );

  const handleDeleteApplication = useCallback(
    async (bundleIdentifier: string) => {
      setActionInProgress(bundleIdentifier);
      try {
        await removeApplication(bundleIdentifier);
      } catch (err) {
        console.error('Failed to remove application:', err);
      } finally {
        setActionInProgress(null);
      }
    },
    [removeApplication, setActionInProgress]
  );

  const handleRefresh = useCallback(async () => {
    clearError();
    await loadAllApplications();
  }, [clearError, loadAllApplications]);

  const handleStartEdit = useCallback(
    (app: AvailableApplicationInfo) => {
      setEditingBundleId(app.bundleIdentifier);
      setEditedName(nameChanges[app.bundleIdentifier] ?? app.applicationName);
    },
    [nameChanges]
  );

  const handleCancelEdit = useCallback(() => {
    setEditingBundleId(null);
    setEditedName('');
  }, []);

  const handleSaveEdit = useCallback(
    (bundleIdentifier: string) => {
      if (editedName.trim() === '') {
        handleCancelEdit();
        return;
      }

      setNameChanges((prev) => ({
        ...prev,
        [bundleIdentifier]: editedName,
      }));

      setEditingBundleId(null);
      setEditedName('');
    },
    [editedName, handleCancelEdit]
  );

  const toAdd = useMemo(() => {
    const originalSet = new Set(
      (originalApps ?? []).filter((o) => o.isInDatabase).map((o) => o.bundleIdentifier)
    );
    return (updatedApps ?? []).filter((u) => !originalSet.has(u.bundleIdentifier));
  }, [originalApps, updatedApps]);

  const toRemove = useMemo(() => {
    const updatedSet = new Set((updatedApps ?? []).map((u) => u.bundleIdentifier));
    return (originalApps ?? []).filter(
      (o) => !updatedSet.has(o.bundleIdentifier) && o.isInDatabase
    );
  }, [originalApps, updatedApps]);

  const performSave = useCallback(async () => {
    try {
      console.log('removing applications', toRemove);
      await Promise.all(toRemove.map((r) => handleDeleteApplication(r.bundleIdentifier)));
      console.log('adding applications', toAdd);
      await Promise.all(
        toAdd.map((a) => {
          const customName = nameChanges[a.bundleIdentifier];
          return handleSaveApplication(a, customName);
        })
      );

      const toUpdateName = Object.entries(nameChanges).filter(([bundleId, newName]) => {
        const app = originalApps?.find((a) => a.bundleIdentifier === bundleId);
        return app?.isInDatabase && app.applicationName !== newName;
      });
      console.log('updating names for', toUpdateName);
      const nameUpdates = toUpdateName.map(([bundleId, newName]) =>
        updateApplicationName(bundleId, newName)
      );

      await Promise.all(nameUpdates);
      await refreshApplications();
      close();
    } catch (err) {
      console.error('failed to save', err);
    }
  }, [
    toRemove,
    toAdd,
    nameChanges,
    close,
    handleDeleteApplication,
    handleSaveApplication,
    originalApps,
    updateApplicationName,
    refreshApplications,
  ]);

  const handleBulkSave = useCallback(() => {
    void performSave();
  }, [performSave]);

  const rows = useMemo(
    () =>
      allApplications
        .sort(
          (a, b) =>
            (originalSort?.indexOf(a.bundleIdentifier) ?? 0) -
            (originalSort?.indexOf(b.bundleIdentifier) ?? 0)
        )
        .map((app) => {
          const isSaved = originalApps?.some(
            (a) => a.bundleIdentifier === app.bundleIdentifier && a.isInDatabase
          );
          const isStagedForAdd =
            updatedApps?.some((a) => a.bundleIdentifier === app.bundleIdentifier) && !isSaved;
          const isStagedForRemove =
            !updatedApps?.some((u) => u.bundleIdentifier === app.bundleIdentifier) &&
            app.isInDatabase;

          const displayName = nameChanges[app.bundleIdentifier] ?? app.applicationName;
          const hasNameChanged =
            nameChanges[app.bundleIdentifier] &&
            nameChanges[app.bundleIdentifier] !== app.applicationName;
          const isInUpdatedApps = updatedApps?.some(
            (a) => a.bundleIdentifier === app.bundleIdentifier
          );

          return (
            <Table.Tr key={app.bundleIdentifier}>
              <Table.Td>
                {editingBundleId === app.bundleIdentifier ? (
                  <TextInput
                    value={editedName}
                    onChange={(e) => setEditedName(e.currentTarget.value)}
                    onBlur={handleCancelEdit}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        handleSaveEdit(app.bundleIdentifier);
                      }
                    }}
                    size="sm"
                    autoFocus
                    rightSection={
                      <Tooltip label="Save">
                        <ActionIcon
                          variant="subtle"
                          color="blue"
                          onMouseDown={(e) => {
                            e.preventDefault();
                            handleSaveEdit(app.bundleIdentifier);
                          }}
                        >
                          <IconCheck size={16} />
                        </ActionIcon>
                      </Tooltip>
                    }
                  />
                ) : isInUpdatedApps ? (
                  <Tooltip
                    label={hasNameChanged ? `Original: ${app.applicationName}` : undefined}
                    disabled={!hasNameChanged}
                  >
                    <Group
                      gap="xs"
                      onClick={() => handleStartEdit(app)}
                      className={classes.clickableNameGroup}
                    >
                      <Text
                        fw={500}
                        c={hasNameChanged ? 'dimmed' : undefined}
                        td={hasNameChanged ? 'underline' : undefined}
                      >
                        {displayName}
                      </Text>
                      <ActionIcon variant="subtle" color="blue" size="sm">
                        <IconPencil size={14} />
                      </ActionIcon>
                    </Group>
                  </Tooltip>
                ) : (
                  <Text fw={500}>{displayName}</Text>
                )}
              </Table.Td>
              <Table.Td>
                <Text size="sm" c="dimmed">
                  {app.bundleIdentifier}
                </Text>
              </Table.Td>
              <Table.Td>
                {!isStagedForAdd && app.isInDatabase && !isStagedForRemove && (
                  <Badge color="green" variant="light" leftSection={<IconCheck size={14} />}>
                    In Database
                  </Badge>
                )}
                {isStagedForAdd && (
                  <Badge color="blue" variant="light" leftSection={<IconCheck size={14} />}>
                    To be added
                  </Badge>
                )}

                {!app.isInDatabase && !isStagedForAdd && (
                  <Badge color="gray" variant="light">
                    Not Added
                  </Badge>
                )}
                {isStagedForRemove && (
                  <Badge color="red" variant="light">
                    To be removed
                  </Badge>
                )}
              </Table.Td>
              <Table.Td>
                <Group gap="xs" justify="flex-end">
                  {((isSaved && !isStagedForRemove) || isStagedForAdd) && (
                    <Tooltip label="Remove from available applications">
                      <ActionIcon
                        color="red"
                        variant="light"
                        onClick={() => {
                          setUpdatedApps((updated) =>
                            updated?.filter((u) => u.bundleIdentifier !== app.bundleIdentifier)
                          );
                        }}
                        loading={actionInProgress === app.bundleIdentifier}
                      >
                        <IconTrash size={18} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                  {(isStagedForRemove || (!isSaved && !isStagedForAdd)) && (
                    <Tooltip label="Add to available applications">
                      <ActionIcon
                        color="blue"
                        variant="light"
                        onClick={() => {
                          setUpdatedApps((updated) => [...(updated ?? []), app]);
                        }}
                        loading={actionInProgress === app.bundleIdentifier}
                      >
                        <IconPlus size={18} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                </Group>
              </Table.Td>
            </Table.Tr>
          );
        }),
    [
      allApplications,
      originalSort,
      originalApps,
      updatedApps,
      nameChanges,
      editingBundleId,
      editedName,
      handleCancelEdit,
      classes.clickableNameGroup,
      actionInProgress,
      handleSaveEdit,
      handleStartEdit,
    ]
  );

  return (
    <Modal
      title={
        <Group justify="space-between">
          <Title order={3}>Application Audio Manager</Title>
          <Tooltip label="Refresh application list">
            <Button
              leftSection={<IconRefresh size={16} />}
              variant="light"
              onClick={() => {
                void handleRefresh();
              }}
              loading={isLoading}
            >
              Refresh
            </Button>
          </Tooltip>
        </Group>
      }
      opened={opened}
      size="xl"
      onClose={close}
      className={classes.card}
      padding="lg"
    >
      <Flex direction="column" style={{ height: '70vh' }}>
        <Text size="sm" mb="sm" c="dimmed">
          Applications shown here are detected via ScreenCaptureKit. Add applications to enable
          audio capture from them.
        </Text>

        <ScrollArea style={{ flex: 1 }}>
          <Stack gap="md" mr="sm">
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
              <Table stickyHeader className={classes.table} striped highlightOnHover>
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
          </Stack>
        </ScrollArea>

        <Group justify="flex-end" mt="sm">
          <ButtonGroup>
            <Button onClick={close} variant="outline">
              Cancel
            </Button>
            <Button onClick={handleBulkSave} variant="primary">
              Save
            </Button>
          </ButtonGroup>
        </Group>
      </Flex>
    </Modal>
  );
});

ApplicationAudioManager.displayName = 'ApplicationAudioManager';
