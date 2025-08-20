import {
  Card,
  Stack,
  Group,
  Title,
  Select,
  Box,
  Text,
  Button,
  ActionIcon,
  Alert,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconDevices,
  IconRefresh,
  IconPlayerPlay,
  IconPlayerPause,
} from '@tabler/icons-react';
import { memo } from 'react';
import { VUMeter } from './VUMeter';

type AudioDevice = {
  deviceId: string;
  label: string;
};

type AudioControlsCardProps = {
  audioDevices: AudioDevice[];
  selectedDevice: string;
  audioLevel: number;
  isConnected: boolean;
  isStreaming: boolean;
  isRefreshingDevices: boolean;
  onDeviceChange: (deviceId: string) => void;
  onRefreshDevices: () => void;
  onStartStreaming: () => void;
  onStopStreaming: () => void;
};

const useStyles = createStyles((theme) => ({
  audioCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  vuMeterContainer: {
    textAlign: 'center',
  },

  levelText: {
    minWidth: 50,
    textAlign: 'center',
  },
}));

export const AudioControlsCard = memo<AudioControlsCardProps>(({
  audioDevices,
  selectedDevice,
  audioLevel,
  isConnected,
  isStreaming,
  isRefreshingDevices,
  onDeviceChange,
  onRefreshDevices,
  onStartStreaming,
  onStopStreaming,
}) => {
  const { classes } = useStyles();

  return (
    <Card className={classes.audioCard} padding="lg" withBorder>
      <Stack gap="md">
        <Group justify="space-between">
          <Title order={3} c="blue.4">
            Audio Controls
          </Title>
          <ActionIcon
            onClick={onRefreshDevices}
            loading={isRefreshingDevices}
            variant="light"
            color="blue"
          >
            <IconRefresh size={16} />
          </ActionIcon>
        </Group>

        <Select
          label="Audio Input Device"
          placeholder="Select audio device"
          value={selectedDevice}
          onChange={(value) => onDeviceChange(value || '')}
          data={audioDevices.map((device) => ({
            value: device.deviceId,
            label: device.label,
          }))}
          leftSection={<IconDevices size={16} />}
        />
        
        {selectedDevice === 'system-audio' && (
          <Alert color="yellow" variant="light" size="sm">
            Note: System audio capture may require additional permissions and may not work in all browsers.
          </Alert>
        )}

        <Box className={classes.vuMeterContainer}>
          <Text size="sm" fw={500} mb="xs">
            Audio Level
          </Text>
          <VUMeter level={audioLevel} />
          <Text size="xs" c="dimmed" ta="center" mt="xs" className={classes.levelText}>
            {Math.round(audioLevel)} dB
          </Text>
        </Box>

        {isConnected && (
          <Group grow>
            {!isStreaming ? (
              <Button
                onClick={onStartStreaming}
                leftSection={<IconPlayerPlay size={16} />}
                color="green"
              >
                Start Streaming
              </Button>
            ) : (
              <Button
                onClick={onStopStreaming}
                leftSection={<IconPlayerPause size={16} />}
                color="red"
                variant="light"
              >
                Stop Streaming
              </Button>
            )}
          </Group>
        )}
      </Stack>
    </Card>
  );
});

AudioControlsCard.displayName = 'AudioControlsCard';