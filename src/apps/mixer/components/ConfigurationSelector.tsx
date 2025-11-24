import { Card, Select, Stack, Text, Alert, Loader } from '@mantine/core';
import { IconAlertCircle } from '@tabler/icons-react';
import { useCallback, useEffect } from 'react';

import { useConfigurationStore } from '../stores/mixer-store';

type ConfigurationSelectorProps = {
  onConfigurationSelect?: (configId: string) => void;
};

export const ConfigurationSelector = ({ onConfigurationSelect }: ConfigurationSelectorProps) => {
  const {
    reusableConfigurations,
    activeSession,
    isLoading,
    error,
    loadConfigurations,
    selectConfiguration,
    clearError,
  } = useConfigurationStore();

  // Load configurations on mount
  useEffect(() => {
    void loadConfigurations();
  }, [loadConfigurations]);

  const handleConfigurationSelect = useCallback(
    async (configId: string | null) => {
      if (!configId) {return;}

      await selectConfiguration(configId);
      onConfigurationSelect?.(configId);
    },
    [onConfigurationSelect, selectConfiguration]
  );

  const selectData = reusableConfigurations.map((completeData) => ({
    value: completeData.configuration.id,
    label: completeData.configuration.name,
    description: completeData.configuration.description,
  }));

  // Find the currently selected value based on active session's linked reusable config
  const selectedValue = activeSession?.configuration.reusableConfigurationId || null;

  return (
    <Card withBorder p="md">
      <Stack gap="sm">
        {/* Configuration Selector */}
        <Select
          label="Load Configuration"
          placeholder="Select a reusable configuration..."
          data={selectData}
          value={selectedValue}
          searchable
          clearable
          disabled={isLoading}
          onChange={handleConfigurationSelect}
          leftSection={isLoading ? <Loader size="xs" /> : undefined}
        />

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

        {/* Empty State */}
        {reusableConfigurations.length === 0 && !isLoading && !error && (
          <Text size="sm" c="dimmed" ta="center">
            No configurations found. Create one by saving your current session.
          </Text>
        )}
      </Stack>
    </Card>
  );
};
