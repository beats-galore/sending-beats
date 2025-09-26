// Master section with output routing and master VU meters
import { Paper, Grid, Stack, Title, Text, Group, Select, ActionIcon, Center } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconRefresh } from '@tabler/icons-react';
import { memo, useCallback, useMemo } from 'react';

import {
  useMasterSectionData,
  useMasterLevels,
  useAudioMetrics,
  useAudioDevices,
} from '../../hooks';
import { useConfigurationStore } from '../../stores/mixer-store';
import { audioService } from '../../services';
import { VUMeter, AudioSlider } from '../ui';

const useStyles = createStyles(() => ({
  responsiveGrid: {
    width: '100%',
    maxWidth: '100%',
  },
}));

export const MasterSection = memo(() => {
  const { classes } = useStyles();
  const { mixerConfig, setMasterGain, setMasterOutputDevice } = useMasterSectionData();
  const masterLevels = useMasterLevels();
  const metrics = useAudioMetrics();
  const { outputDevices, refreshDevices } = useAudioDevices();
  const { activeSession } = useConfigurationStore();

  // Find the configured output device from the active session
  const configuredOutputDevice = useMemo(() => {
    if (!activeSession?.configured_devices) return null;
    return activeSession.configured_devices.find(
      (device) => !device.is_input // Output devices have is_input = false
    );
  }, [activeSession]);

  const handleOutputDeviceChange = useCallback(
    async (deviceId: string) => {
      try {
        await setMasterOutputDevice(deviceId);
      } catch (error) {
        console.error('Failed to set output device:', error);
      }
    },
    [setMasterOutputDevice]
  );

  const handleMasterGainChange = useCallback(
    (gain: number) => {
      setMasterGain(gain);
    },
    [setMasterGain]
  );

  const outputDeviceOptions = useMemo(() => {
    // Check for duplicates and deduplication if needed
    const deviceMap = new Map();
    const duplicateIds: string[] = [];

    outputDevices.forEach((device) => {
      if (deviceMap.has(device.id)) {
        duplicateIds.push(device.id);
        console.warn('🚨 Duplicate output device ID detected:', device.id, device.name);
      }
      deviceMap.set(device.id, device);
    });

    if (duplicateIds.length > 0) {
      console.error('🚨 Found duplicate output device IDs:', duplicateIds);
    }

    // Return unique devices only
    const uniqueDevices = Array.from(deviceMap.values());
    const options = uniqueDevices.map((device) => ({
      value: device.id,
      label: device.name + (device.is_default ? ' (Default)' : ''),
    }));

    // Add configured output device if it's not in the available devices list (missing/unplugged)
    if (configuredOutputDevice) {
      const isDeviceAvailable = outputDevices.some(device => device.id === configuredOutputDevice.device_identifier);
      if (!isDeviceAvailable) {
        const deviceName = configuredOutputDevice.device_name || configuredOutputDevice.device_identifier;
        options.unshift({
          value: configuredOutputDevice.device_identifier,
          label: `${deviceName} (unavailable)`,
          disabled: true, // Disable the option so it can't be selected
        });
      }
    }

    return options;
  }, [outputDevices, configuredOutputDevice]);

  return (
    <Paper p="lg" withBorder radius="md">
      <Stack gap="lg">
        <Title order={3} c="blue">
          Master Section
        </Title>

        <Grid gutter="md" className={classes.responsiveGrid}>
          {/* Master VU Meters */}
          <Grid.Col span={{ base: 12, md: 4 }}>
            <Stack align="center">
              <Title order={4}>Master Levels</Title>
              <Group justify="center" gap="lg">
                <Center>
                  <Stack align="center">
                    <Text size="sm" c="dimmed">
                      L
                    </Text>
                    <VUMeter
                      peakLevel={masterLevels?.left?.peak_level || 0}
                      rmsLevel={masterLevels?.left?.rms_level || 0}
                      height={200}
                      width={20}
                    />
                    <Text size="xs" c="dimmed">
                      {(masterLevels?.left?.peak_level || 0).toFixed(3)}
                    </Text>
                  </Stack>
                </Center>

                <Center>
                  <Stack align="center">
                    <Text size="sm" c="dimmed">
                      R
                    </Text>
                    <VUMeter
                      peakLevel={masterLevels?.right?.peak_level || 0}
                      rmsLevel={masterLevels?.right?.rms_level || 0}
                      height={200}
                      width={20}
                    />
                    <Text size="xs" c="dimmed">
                      {(masterLevels?.right?.peak_level || 0).toFixed(3)}
                    </Text>
                  </Stack>
                </Center>
              </Group>
            </Stack>
          </Grid.Col>

          {/* Master Controls */}
          <Grid.Col span={{ base: 12, md: 4 }}>
            <Stack gap="lg">
              <Title order={4}>Controls</Title>

              {/* Master Gain */}
              <AudioSlider
                label="Master Gain"
                value={mixerConfig?.master_gain || 0}
                min={-60}
                max={12}
                step={0.5}
                unit="dB"
                onChange={handleMasterGainChange}
              />

              {/* Output Device Selection */}
              <Stack gap="xs">
                <Group justify="space-between">
                  <Text size="sm" c="dimmed">
                    Master Output Device
                  </Text>
                  <ActionIcon
                    variant="light"
                    onClick={refreshDevices}
                    title="Refresh devices"
                    size="sm"
                  >
                    <IconRefresh size={16} />
                  </ActionIcon>
                </Group>
                <Select
                  placeholder="Select output device..."
                  data={outputDeviceOptions}
                  value={configuredOutputDevice?.device_identifier || null}
                  onChange={(value) => value && handleOutputDeviceChange(value)}
                />
              </Stack>
            </Stack>
          </Grid.Col>

          {/* Audio Metrics */}
          <Grid.Col span={{ base: 12, md: 4 }}>
            <Stack gap="sm">
              <Title order={4}>Audio Metrics</Title>
              {metrics ? (
                <Stack gap="xs">
                  <Group justify="space-between">
                    <Text size="sm" c="dimmed">
                      CPU Usage:
                    </Text>
                    <Text size="sm">{metrics.cpu_usage.toFixed(1)}%</Text>
                  </Group>
                  <Group justify="space-between">
                    <Text size="sm" c="dimmed">
                      Sample Rate:
                    </Text>
                    <Text size="sm">{metrics.sample_rate}Hz</Text>
                  </Group>
                  <Group justify="space-between">
                    <Text size="sm" c="dimmed">
                      Latency:
                    </Text>
                    <Text size="sm">{metrics.latency_ms.toFixed(1)}ms</Text>
                  </Group>
                  <Group justify="space-between">
                    <Text size="sm" c="dimmed">
                      Active Channels:
                    </Text>
                    <Text size="sm">{metrics.active_channels}</Text>
                  </Group>
                  <Group justify="space-between">
                    <Text size="sm" c="dimmed">
                      Buffer Underruns:
                    </Text>
                    <Text size="sm" c={metrics.buffer_underruns > 0 ? 'red' : 'green'}>
                      {metrics.buffer_underruns}
                    </Text>
                  </Group>
                  <Group justify="space-between">
                    <Text size="sm" c="dimmed">
                      Buffer Overruns:
                    </Text>
                    <Text size="sm" c={metrics.buffer_overruns > 0 ? 'red' : 'green'}>
                      {metrics.buffer_overruns}
                    </Text>
                  </Group>
                </Stack>
              ) : (
                <Text c="dimmed">No metrics available</Text>
              )}
            </Stack>
          </Grid.Col>
        </Grid>
      </Stack>
    </Paper>
  );
});
