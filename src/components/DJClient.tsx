// Professional DJ Streaming Client - Modernized with Mantine
import {
  Box,
  Stack,
  Group,
  Title,
  Alert,
  Grid,
  LoadingOverlay,
  Badge,
  ScrollArea,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconAlertCircle, IconWifi, IconWifiOff } from '@tabler/icons-react';
import { memo, useState, useRef, useEffect, useCallback } from 'react';

import { useStreamingStatus, useStreamingControls } from '../hooks';
import { DEFAULT_SAMPLE_RATE_HZ, type SampleRate } from '../utils/constants';

import {
  StreamStatusCard,
  StreamConfigurationCard,
  StreamDiagnosticsCard,
  AudioControlsCard,
  MetadataCard,
  VariableBitrateCard,
} from './dj';

type AudioDevice = {
  deviceId: string;
  label: string;
};

type StreamSettings = {
  bitrate: number;
  sampleRate: number;
  channels: number;
};

type StreamConfigUI = {
  icecast_url: string;
  mount_point: string;
  username: string;
  password: string;
  bitrate: number;
  sample_rate: SampleRate;
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
    height: '100vh',
    display: 'flex',
    flexDirection: 'column',
  },

  scrollContent: {
    flex: 1,
    paddingRight: theme.spacing.sm,
  },

  statusIndicator: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },
}));

const DJClient = memo(() => {
  const { classes } = useStyles();

  // Use new streaming hooks
  const {
    status: streamingStatus,
    isLoading: isStatusLoading,
    error: statusError,
    actions: statusActions,
  } = useStreamingStatus();
  const { state: controlsState, actions: controlsActions } = useStreamingControls();

  const [selectedDevice, setSelectedDevice] = useState<string>('');
  const [audioDevices, setAudioDevices] = useState<AudioDevice[]>([]);
  const [streamSettings, setStreamSettings] = useState<StreamSettings>({
    bitrate: streamingStatus?.bitrate_info.current_bitrate ?? 192,
    sampleRate: DEFAULT_SAMPLE_RATE_HZ,
    channels: 2,
  });

  const [availableBitrates, setAvailableBitrates] = useState<number[]>([]);
  const [streamConfigUI, setStreamConfigUI] = useState<StreamConfigUI>({
    icecast_url: 'http://localhost:8000',
    mount_point: '/live',
    username: 'source',
    password: '',
    bitrate: 192,
    sample_rate: DEFAULT_SAMPLE_RATE_HZ,
    channels: 2,
  });
  const [metadata, setMetadata] = useState({
    title: '',
    artist: '',
    album: '',
  });
  const [audioLevel, setAudioLevel] = useState(0);
  const [isRefreshingDevices, setIsRefreshingDevices] = useState(false);

  // Derived state from new streaming status
  const isConnected = streamingStatus?.is_connected ?? false;
  const isStreaming = streamingStatus?.is_streaming ?? false;
  const isConnecting = controlsState.isConnecting;
  const error = controlsState.error ?? statusError;

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
      controlsActions.clearError();
      console.error('Failed to get audio devices:', err);
    } finally {
      setIsRefreshingDevices(false);
    }
  }, [selectedDevice]);

  // Load available bitrates on mount
  useEffect(() => {
    const loadBitrates = async () => {
      try {
        const bitrates = await statusActions.getAvailableBitrates();
        setAvailableBitrates(bitrates);
      } catch (err) {
        console.error('Failed to load available bitrates:', err);
      }
    };

    void loadBitrates();
  }, [statusActions]);

  // Sync current bitrate from streaming status
  useEffect(() => {
    if (
      streamingStatus?.bitrate_info.current_bitrate &&
      streamingStatus.bitrate_info.current_bitrate !== streamSettings.bitrate
    ) {
      setStreamSettings((prev) => ({
        ...prev,
        bitrate: streamingStatus.bitrate_info.current_bitrate,
      }));
    }
  }, [streamingStatus?.bitrate_info.current_bitrate, streamSettings.bitrate]);

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

  // Handle bitrate changes from configuration
  const handleBitrateChange = useCallback(
    async (newBitrate: number) => {
      try {
        await statusActions.setBitrate(newBitrate);
        setStreamSettings((prev) => ({ ...prev, bitrate: newBitrate }));
      } catch (err) {
        console.error('Failed to set bitrate:', err);
      }
    },
    [statusActions]
  );

  // Handle variable bitrate changes
  const handleVariableBitrateChange = useCallback(
    async (enabled: boolean, quality: number) => {
      try {
        await statusActions.setVariableBitrate(enabled, quality);
      } catch (err) {
        console.error('Failed to set variable bitrate:', err);
      }
    },
    [statusActions]
  );

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
      if (!selectedDevice) {return;}

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
      console.error('Failed to start audio monitoring:', err);
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
      controlsActions.clearError();

      // Parse URL to get host and port
      const url = new URL(streamConfigUI.icecast_url);
      const host = url.hostname;
      const port = parseInt(url.port) || 8000;

      // Initialize streaming with new backend
      await controlsActions.initialize({
        server_host: host,
        server_port: port,
        mount_point: streamConfigUI.mount_point,
        password: streamConfigUI.password,
        stream_name: 'Sendin Beats Live Stream',
        bitrate: streamSettings.bitrate,
      });

      await startAudioMonitoring();
    } catch (err) {
      console.error('Failed to connect to stream:', err);
    }
  }, [streamConfigUI, streamSettings, controlsActions]);

  const disconnectFromStream = useCallback(async () => {
    try {
      await controlsActions.stopStreaming();
      stopAudioMonitoring();
    } catch (err) {
      console.error('Failed to disconnect:', err);
    }
  }, [controlsActions]);

  const startStreaming = useCallback(async () => {
    try {
      await controlsActions.startStreaming();

      // Set up audio data sender (legacy approach for browser audio)
      streamSenderRef.current = async (audioData: Uint8Array) => {
        // Note: In the new architecture, audio comes from the mixer
        // This is kept for backward compatibility but may not be used
        console.debug('Browser audio data available:', audioData.length);
      };
    } catch (err) {
      console.error('Failed to start streaming:', err);
    }
  }, [controlsActions]);

  const stopStreaming = useCallback(async () => {
    try {
      await controlsActions.stopStreaming();
      streamSenderRef.current = null;
    } catch (err) {
      console.error('Failed to stop streaming:', err);
    }
  }, [controlsActions]);

  const updateMetadata = useCallback(async () => {
    if (!metadata.title || !metadata.artist) {return;}

    try {
      await controlsActions.updateMetadata(metadata.title, metadata.artist);
      console.debug('Metadata updated successfully');
    } catch (err) {
      console.error('Failed to update metadata:', err);
    }
  }, [metadata, controlsActions]);

  return (
    <Box className={classes.container} pos="relative">
      <LoadingOverlay visible={isConnecting} />

      {/* Fixed Header */}
      <Group justify="space-between" align="center" mb="lg">
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

      {/* Scrollable Content */}
      <ScrollArea className={classes.scrollContent}>
        <Stack gap="lg">
          {error && (
            <Alert icon={<IconAlertCircle size={16} />} title="Error" color="red" variant="light">
              {error}
            </Alert>
          )}

          <StreamStatusCard
            streamStatus={{
              is_connected: isConnected,
              is_streaming: isStreaming,
              current_listeners: 0,
              peak_listeners: 0,
              stream_duration: streamingStatus?.uptime_seconds ?? 0,
              bitrate: streamingStatus?.bitrate_info.current_bitrate ?? 0,
              error_message: error ?? undefined,
            }}
          />

          {/* New Advanced Diagnostics */}
          {streamingStatus && (
            <StreamDiagnosticsCard
              connectionDiagnostics={streamingStatus.connection_diagnostics}
              bitrateInfo={streamingStatus.bitrate_info}
              audioStats={streamingStatus.audio_stats}
            />
          )}

          {/* Variable Bitrate Configuration */}
          {streamingStatus && (
            <VariableBitrateCard
              bitrateInfo={streamingStatus.bitrate_info}
              onVariableBitrateChange={(...args) => void handleVariableBitrateChange(...args)}
              disabled={!isConnected}
            />
          )}

          <Grid>
            {/* Stream Configuration */}
            <Grid.Col span={{ base: 12, md: 6 }}>
              <StreamConfigurationCard
                streamConfig={streamConfigUI}
                streamSettings={streamSettings}
                availableBitrates={availableBitrates}
                isConnected={isConnected}
                isConnecting={isConnecting}
                onConfigChange={(c) => void setStreamConfigUI(c)}
                onSettingsChange={(settings) => {
                  setStreamSettings(settings);
                  if (settings.bitrate !== streamSettings.bitrate) {
                    handleBitrateChange(settings.bitrate);
                  }
                }}
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
      </ScrollArea>
    </Box>
  );
});

DJClient.displayName = 'DJClient';

export default DJClient;
