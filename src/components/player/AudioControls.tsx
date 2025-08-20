import {
  Card,
  Stack,
  Center,
  Button,
  Group,
  ActionIcon,
  Text,
  Slider,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconPlayerPlay,
  IconPlayerPause,
  IconVolume,
  IconVolumeOff,
} from '@tabler/icons-react';
import { memo } from 'react';

type StreamStatus = {
  is_connected: boolean;
  is_streaming: boolean;
  current_listeners: number;
  peak_listeners: number;
  stream_duration: number;
  bitrate: number;
  error_message?: string;
};

type AudioControlsProps = {
  isPlaying: boolean;
  volume: number;
  isLoading: boolean;
  streamStatus: StreamStatus | null;
  onPlay: () => void;
  onPause: () => void;
  onVolumeChange: (volume: number) => void;
};

const useStyles = createStyles((theme) => ({
  controlsCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  playButton: {
    width: 64,
    height: 64,
    borderRadius: '50%',
  },

  volumeLabel: {
    minWidth: 50,
  },

  volumeSlider: {
    flex: 1,
  },

  volumeValue: {
    minWidth: 40,
    textAlign: 'center',
  },
}));

export const AudioControls = memo<AudioControlsProps>(({
  isPlaying,
  volume,
  isLoading,
  streamStatus,
  onPlay,
  onPause,
  onVolumeChange,
}) => {
  const { classes } = useStyles();

  return (
    <Card className={classes.controlsCard} padding="lg" withBorder>
      <Stack gap="lg">
        <Center>
          <Button
            className={classes.playButton}
            onClick={isPlaying ? onPause : onPlay}
            disabled={isLoading || !streamStatus?.is_streaming}
            loading={isLoading}
            color={isPlaying ? 'red' : 'green'}
            variant={isPlaying ? 'light' : 'filled'}
            size="xl"
          >
            {isPlaying ? (
              <IconPlayerPause size={24} />
            ) : (
              <IconPlayerPlay size={24} />
            )}
          </Button>
        </Center>

        {/* Volume Control */}
        <Group gap="md" align="center">
          <ActionIcon variant="subtle" color="blue">
            {volume === 0 ? <IconVolumeOff size={16} /> : <IconVolume size={16} />}
          </ActionIcon>
          <Text size="sm" c="dimmed" className={classes.volumeLabel}>
            Volume
          </Text>
          <Slider
            value={volume * 100}
            onChange={(value) => onVolumeChange(value / 100)}
            min={0}
            max={100}
            step={1}
            className={classes.volumeSlider}
            color="blue"
          />
          <Text size="sm" c="dimmed" className={classes.volumeValue}>
            {Math.round(volume * 100)}%
          </Text>
        </Group>
      </Stack>
    </Card>
  );
});

AudioControls.displayName = 'AudioControls';