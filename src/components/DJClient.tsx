// Professional DJ Streaming Client - Modernized with Mantine
import { Box, Stack, Group, Text, Title, Alert, Grid, LoadingOverlay, Badge } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconAlertCircle, IconWifi, IconWifiOff } from '@tabler/icons-react';
import { invoke } from '@tauri-apps/api/core';
import { memo, useState, useRef, useEffect, useCallback } from 'react';

import { StreamStatusCard, StreamConfigurationCard, AudioControlsCard, MetadataCard } from './dj';
import { ErrorBoundary } from './layout';

type AudioDevice = {
  deviceId: string;
  label: string;
};

type StreamSettings = {
  bitrate: number;
  sampleRate: number;
  channels: number;
};

type StreamConfig = {
  icecast_url: string;
  mount_point: string;
  username: string;
  password: string;
  bitrate: number;
  sample_rate: number;
  channels: number;
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

type StreamMetadata = {
  title: string;
  artist: string;
  album?: string;
  genre?: string;
};

const useStyles = createStyles((theme) => ({
  container: {
    padding: theme.spacing.md,
    maxWidth: 1200,
    margin: '0 auto',
  },

  statusIndicator: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },
}));

const DJClient = memo(() => {
  const { classes } = useStyles();
  const [isConnected, setIsConnected] = useState(false);
  const [isStreaming, setIsStreaming] = useState(false);
  const [selectedDevice, setSelectedDevice] = useState<string>('');
  const [audioDevices, setAudioDevices] = useState<AudioDevice[]>([]);
  const [streamSettings, setStreamSettings] = useState<StreamSettings>({
    bitrate: 128,
    sampleRate: 44100,
    channels: 2,
  });
  const [streamConfig, setStreamConfig] = useState<StreamConfig>({
    icecast_url: 'http://localhost:8000',
    mount_point: 'live',
    username: 'source',
    password: '',
    bitrate: 128,
    sample_rate: 44100,
    channels: 2,
  });
  const [metadata, setMetadata] = useState({
    title: '',
    artist: '',
    album: '',
  });
  const [audioLevel, setAudioLevel] = useState(0);
  const [error, setError] = useState<string>('');
  const [streamStatus, setStreamStatus] = useState<StreamStatus | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isRefreshingDevices, setIsRefreshingDevices] = useState(false);

  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const mediaStreamRef = useRef<MediaStream | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const streamIntervalRef = useRef<number | null>(null);
  const audioProcessorRef = useRef<ScriptProcessorNode | null>(null);
  const streamSenderRef = useRef<((data: Uint8Array) => void) | null>(null);

  // Get available audio devices
  const getAudioDevices = useCallback(async () => {
    try {
      setIsRefreshingDevices(true);
      const devices = await navigator.mediaDevices.enumerateDevices();

      // Get all audio input devices (microphones, system audio, etc.)
      const audioInputs = devices
        .filter((device) => device.kind === 'audioinput')
        .map((device) => ({
          deviceId: device.deviceId,
          label: device.label || `Audio Input ${device.deviceId.slice(0, 8)}`,
        }));

      // Add system audio capture option if available
      const systemAudioOption = {
        deviceId: 'system-audio',
        label: 'System Audio (All Sounds)',
      };

      // Combine system audio with detected devices
      const allDevices = [systemAudioOption, ...audioInputs];

      setAudioDevices(allDevices);
      if (allDevices.length > 0 && !selectedDevice) {
        setSelectedDevice(allDevices[0].deviceId);
      }
    } catch (err) {
      setError('Failed to get audio devices');
      console.error(err);
    } finally {
      setIsRefreshingDevices(false);
    }
  }, [selectedDevice]);

  useEffect(() => {
    void getAudioDevices();

    // Listen for device changes
    const handleDeviceChange = () => {
      void getAudioDevices();
    };

    navigator.mediaDevices.addEventListener('devicechange', handleDeviceChange);

    return () => {
      navigator.mediaDevices.removeEventListener('devicechange', handleDeviceChange);
    };
  }, [getAudioDevices]);

  // Update stream status periodically
  useEffect(() => {
    if (isConnected) {
      const updateStatus = async () => {
        try {
          const status = await invoke<StreamStatus>('get_stream_status');
          setStreamStatus(status);
        } catch (err) {
          console.error('Failed to get stream status:', err);
        }
      };

      updateStatus();
      streamIntervalRef.current = window.setInterval(updateStatus, 5000); // Update every 5 seconds

      return () => {
        if (streamIntervalRef.current) {
          window.clearInterval(streamIntervalRef.current);
        }
      };
    }
  }, [isConnected]);

  // Audio level monitoring
  const updateAudioLevel = () => {
    if (analyserRef.current) {
      const dataArray = new Uint8Array(analyserRef.current.frequencyBinCount);
      analyserRef.current.getByteFrequencyData(dataArray);

      const average = dataArray.reduce((sum, value) => sum + value, 0) / dataArray.length;
      setAudioLevel(average);

      animationFrameRef.current = requestAnimationFrame(updateAudioLevel);
    }
  };

  const startAudioMonitoring = async () => {
    try {
      if (!selectedDevice) return;

      let stream: MediaStream;

      if (selectedDevice === 'system-audio') {
        // For system audio capture, we need to use a different approach
        // This will capture all system audio (requires user permission)
        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            sampleRate: streamSettings.sampleRate,
            channelCount: streamSettings.channels,
            // Try to capture system audio
            echoCancellation: false,
            noiseSuppression: false,
            autoGainControl: false,
          },
        });
      } else {
        // Regular microphone/audio input device
        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            deviceId: selectedDevice,
            sampleRate: streamSettings.sampleRate,
            channelCount: streamSettings.channels,
          },
        });
      }

      mediaStreamRef.current = stream;
      audioContextRef.current = new AudioContext();
      analyserRef.current = audioContextRef.current.createAnalyser();

      const source = audioContextRef.current.createMediaStreamSource(stream);
      source.connect(analyserRef.current);

      // Create audio processor for capturing raw audio data
      const processor = audioContextRef.current.createScriptProcessor(4096, 1, 1);
      processor.onaudioprocess = (event) => {
        const inputBuffer = event.inputBuffer;
        const inputData = inputBuffer.getChannelData(0);

        // Convert float32 to int16 for streaming
        const int16Data = new Int16Array(inputData.length);
        for (let i = 0; i < inputData.length; i++) {
          int16Data[i] = Math.max(-32768, Math.min(32767, inputData[i] * 32768));
        }

        // Send audio data to backend for encoding and streaming
        if (streamSenderRef.current && isStreaming) {
          const audioBytes = new Uint8Array(int16Data.buffer);
          streamSenderRef.current(audioBytes);
        }
      };

      source.connect(processor);
      processor.connect(audioContextRef.current.destination);
      audioProcessorRef.current = processor;

      updateAudioLevel();
    } catch (err) {
      setError('Failed to start audio monitoring');
      console.error(err);
    }
  };

  const stopAudioMonitoring = () => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
    }
    if (mediaStreamRef.current) {
      mediaStreamRef.current.getTracks().forEach((track) => track.stop());
    }
    if (audioProcessorRef.current) {
      audioProcessorRef.current.disconnect();
    }
    if (audioContextRef.current) {
      audioContextRef.current.close();
    }
    setAudioLevel(0);
    streamSenderRef.current = null;
  };

  const connectToStream = useCallback(async () => {
    try {
      setIsConnecting(true);
      setError('');

      // Update stream config with current settings
      const config: StreamConfig = {
        ...streamConfig,
        bitrate: streamSettings.bitrate,
        sample_rate: streamSettings.sampleRate,
        channels: streamSettings.channels,
      };

      const status = await invoke<StreamStatus>('connect_to_stream', { config });
      setStreamStatus(status);
      setIsConnected(status.is_connected);

      if (status.is_connected) {
        await startAudioMonitoring();
      } else if (status.error_message) {
        setError(status.error_message);
      }
    } catch (err) {
      setError(`Failed to connect to stream: ${err}`);
      setIsConnected(false);
    } finally {
      setIsConnecting(false);
    }
  }, [streamConfig, streamSettings]);

  const disconnectFromStream = useCallback(async () => {
    try {
      await invoke('disconnect_from_stream');
      stopAudioMonitoring();
      setIsConnected(false);
      setIsStreaming(false);
      setStreamStatus(null);
      setError('');
    } catch (err) {
      setError(`Failed to disconnect: ${err}`);
    }
  }, []);

  const startStreaming = useCallback(async () => {
    if (!isConnected) return;

    try {
      setIsStreaming(true);

      // Set up audio data sender
      streamSenderRef.current = async (audioData: Uint8Array) => {
        try {
          await invoke('start_streaming', { audioData: Array.from(audioData) });
        } catch (err) {
          console.error('Failed to send audio data:', err);
          setIsStreaming(false);
        }
      };
    } catch (err) {
      setError(`Failed to start streaming: ${err}`);
      setIsStreaming(false);
    }
  }, [isConnected]);

  const stopStreaming = useCallback(async () => {
    try {
      await invoke('stop_streaming');
      setIsStreaming(false);
      streamSenderRef.current = null;
    } catch (err) {
      setError(`Failed to stop streaming: ${err}`);
    }
  }, []);

  const updateMetadata = useCallback(async () => {
    if (!metadata.title || !metadata.artist) return;

    try {
      const streamMetadata: StreamMetadata = {
        title: metadata.title,
        artist: metadata.artist,
        album: metadata.album || undefined,
        genre: 'Electronic',
      };

      await invoke('update_metadata', { metadata: streamMetadata });
      console.debug('Metadata updated successfully');
    } catch (err) {
      setError(`Failed to update metadata: ${err}`);
    }
  }, [metadata]);

  return (
    <ErrorBoundary>
      <Box className={classes.container} pos="relative">
        <LoadingOverlay visible={isConnecting} />

        <Stack gap="lg">
          <Group justify="space-between" align="center">
            <Title order={1} c="blue.4">
              DJ Streaming Client
            </Title>
            <Group className={classes.statusIndicator}>
              {isConnected ? (
                <IconWifi size={20} color="#51cf66" />
              ) : (
                <IconWifiOff size={20} color="#fa5252" />
              )}
              <Badge color={isConnected ? 'green' : 'red'} variant="light" size="md">
                {isConnected ? 'Connected' : 'Disconnected'}
              </Badge>
            </Group>
          </Group>

          {error && (
            <Alert icon={<IconAlertCircle size={16} />} title="Error" color="red" variant="light">
              {error}
            </Alert>
          )}

          <StreamStatusCard streamStatus={streamStatus} />

          <Grid>
            {/* Stream Configuration */}
            <Grid.Col span={{ base: 12, md: 6 }}>
              <StreamConfigurationCard
                streamConfig={streamConfig}
                streamSettings={streamSettings}
                isConnected={isConnected}
                isConnecting={isConnecting}
                onConfigChange={setStreamConfig}
                onSettingsChange={setStreamSettings}
                onConnect={connectToStream}
                onDisconnect={disconnectFromStream}
              />
            </Grid.Col>

            {/* Audio Controls */}
            <Grid.Col span={{ base: 12, md: 6 }}>
              <AudioControlsCard
                audioDevices={audioDevices}
                selectedDevice={selectedDevice}
                audioLevel={audioLevel}
                isConnected={isConnected}
                isStreaming={isStreaming}
                isRefreshingDevices={isRefreshingDevices}
                onDeviceChange={setSelectedDevice}
                onRefreshDevices={getAudioDevices}
                onStartStreaming={startStreaming}
                onStopStreaming={stopStreaming}
              />
            </Grid.Col>
          </Grid>

          {/* Metadata Section */}
          <MetadataCard
            metadata={metadata}
            onMetadataChange={setMetadata}
            onUpdateMetadata={updateMetadata}
          />
        </Stack>
      </Box>
    </ErrorBoundary>
  );
});

DJClient.displayName = 'DJClient';

export default DJClient;
