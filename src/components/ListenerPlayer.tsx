//  Listener Player - Modernized with Mantine
import { Box, Stack, Group, Title, Alert, Grid, LoadingOverlay, Badge, Card } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconAlertCircle,
  IconWifi,
  IconWifiOff,
  IconUsers,
  IconClock,
  IconBroadcast,
} from '@tabler/icons-react';
import { invoke } from '@tauri-apps/api/core';
import { memo, useState, useEffect, useRef, useCallback } from 'react';

import { StatCard, NowPlayingCard, AudioControls, StreamInfoCard } from './player';

type StreamMetadata = {
  title: string;
  artist: string;
  album?: string;
  genre?: string;
};

type StreamStatus = {
  is_connected: boolean;
  is_streaming: boolean;
  current_listeners: number;
  peak_listeners: number;
  stream_duration: number;
  bitrate: number;
  error_message?: string;
};

const useStyles = createStyles((theme) => ({
  container: {
    padding: theme.spacing.md,
    maxWidth: 1000,
    margin: '0 auto',
  },

  statusCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  statusIndicator: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },
}));

const formatTime = (seconds: number): string => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }
  return `${minutes}:${secs.toString().padStart(2, '0')}`;
};

const ListenerPlayer = memo(() => {
  const { classes } = useStyles();
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentMetadata] = useState<StreamMetadata | null>(null);
  const [streamStatus, setStreamStatus] = useState<StreamStatus | null>(null);
  const [volume, setVolume] = useState(0.8);
  const [error, setError] = useState<string>('');
  const [isLoading, setIsLoading] = useState(false);

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const streamUrl = 'http://localhost:8000/live'; // Icecast stream URL

  // Update stream status periodically
  useEffect(() => {
    const updateStatus = async () => {
      try {
        const status = await invoke<StreamStatus>('get_stream_status');
        setStreamStatus(status);
      } catch (err) {
        console.error('Failed to get stream status:', err);
      }
    };

    updateStatus();
    const interval = setInterval(updateStatus, 5000); // Update every 5 seconds

    return () => clearInterval(interval);
  }, []);

  const handlePlay = useCallback(async () => {
    if (!audioRef.current) {
      return;
    }

    try {
      setIsLoading(true);
      setError('');

      // Set the stream URL
      audioRef.current.src = streamUrl;

      // Set volume
      audioRef.current.volume = volume;

      // Start playing
      await audioRef.current.play();
      setIsPlaying(true);
    } catch (err) {
      setError(`Failed to start playback: ${err}`);
      setIsPlaying(false);
    } finally {
      setIsLoading(false);
    }
  }, [streamUrl, volume]);

  const handlePause = useCallback(() => {
    if (audioRef.current) {
      audioRef.current.pause();
      setIsPlaying(false);
    }
  }, []);

  const handleVolumeChange = useCallback((newVolume: number) => {
    setVolume(newVolume);
    if (audioRef.current) {
      audioRef.current.volume = newVolume;
    }
  }, []);

  const handleAudioError = useCallback(() => {
    setError('Failed to load audio stream. Please check your connection.');
    setIsPlaying(false);
  }, []);

  const handleAudioLoadStart = useCallback(() => {
    setIsLoading(true);
  }, []);

  const handleAudioCanPlay = useCallback(() => {
    setIsLoading(false);
  }, []);

  return (
    <Box className={classes.container} pos="relative">
      <LoadingOverlay visible={isLoading} />

      <Stack gap="lg">
        <Group justify="space-between" align="center">
          <Title order={1} c="blue.4">
            Sendin Beats Radio
          </Title>
          <Group className={classes.statusIndicator}>
            {streamStatus?.is_streaming ? (
              <IconWifi size={20} color="#51cf66" />
            ) : (
              <IconWifiOff size={20} color="#fa5252" />
            )}
            <Badge color={streamStatus?.is_streaming ? 'green' : 'red'} variant="light" size="md">
              {streamStatus?.is_streaming ? 'LIVE' : 'OFFLINE'}
            </Badge>
          </Group>
        </Group>

        {error && (
          <Alert
            icon={<IconAlertCircle size={16} />}
            title="Playback Error"
            color="red"
            variant="light"
          >
            {error}
          </Alert>
        )}

        {/* Stream Status */}
        {streamStatus && (
          <Card className={classes.statusCard} padding="lg" withBorder>
            <Grid>
              <Grid.Col span={{ base: 6, md: 3 }}>
                <StatCard
                  icon={<IconUsers size={16} />}
                  value={streamStatus.current_listeners}
                  label="Current Listeners"
                  color="blue"
                />
              </Grid.Col>
              <Grid.Col span={{ base: 6, md: 3 }}>
                <StatCard
                  icon={<IconUsers size={16} />}
                  value={streamStatus.peak_listeners}
                  label="Peak Listeners"
                  color="orange"
                />
              </Grid.Col>
              <Grid.Col span={{ base: 6, md: 3 }}>
                <StatCard
                  icon={<IconClock size={16} />}
                  value={formatTime(streamStatus.stream_duration)}
                  label="Stream Duration"
                  color="green"
                />
              </Grid.Col>
              <Grid.Col span={{ base: 6, md: 3 }}>
                <StatCard
                  icon={<IconBroadcast size={16} />}
                  value={`${streamStatus.bitrate} kbps`}
                  label="Bitrate"
                  color="purple"
                />
              </Grid.Col>
            </Grid>
          </Card>
        )}

        {/* Now Playing */}
        <NowPlayingCard currentMetadata={currentMetadata} />

        {/* Audio Controls */}
        <AudioControls
          isPlaying={isPlaying}
          volume={volume}
          isLoading={isLoading}
          streamStatus={streamStatus}
          onPlay={handlePlay}
          onPause={handlePause}
          onVolumeChange={handleVolumeChange}
        />

        {/* Stream Info */}
        <StreamInfoCard streamUrl={streamUrl} streamStatus={streamStatus} />
      </Stack>

      {/* Hidden audio element */}
      <audio
        ref={audioRef}
        onError={handleAudioError}
        onLoadStart={handleAudioLoadStart}
        onCanPlay={handleAudioCanPlay}
        preload="none"
      />
    </Box>
  );
});

ListenerPlayer.displayName = 'ListenerPlayer';

export default ListenerPlayer;
