import {
  Card,
  Stack,
  Group,
  Title,
  Button,
  Badge,
  Text,
  Progress,
  Select,
  Switch,
  NumberInput,
  Tooltip,
  Alert,
  TextInput,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconCircleFilled,
  IconPlayerStop,
  IconSettings,
  IconFileMusic,
  IconAlertCircle,
  IconFolder,
} from '@tabler/icons-react';
import { invoke } from '@tauri-apps/api/core';
import { memo, useState, useCallback, useEffect } from 'react';

import { useRecording } from '../../hooks/use-recording';

import type { RecordingConfig, RecordingFormat } from '../../hooks/use-recording';

type RecordingControlsCardProps = {
  disabled?: boolean;
};

const useStyles = createStyles((theme) => ({
  recordingCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  recordingButton: {
    height: 60,
    fontSize: theme.fontSizes.lg,
    fontWeight: 700,
  },

  statusDisplay: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.sm,
  },

  progressBar: {
    height: 8,
  },

  configSection: {
    backgroundColor: theme.colors.dark[6],
    padding: theme.spacing.md,
    borderRadius: theme.radius.md,
    border: `1px solid ${theme.colors.dark[4]}`,
  },
}));

const formatDuration = (seconds: number): string => {
  const hrs = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (hrs > 0) {
    return `${hrs}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
};

const getRecordingFormatOptions = () => [
  { value: 'wav', label: 'WAV (Uncompressed)' },
  { value: 'mp3_320', label: 'MP3 320kbps (High Quality)' },
  { value: 'mp3_192', label: 'MP3 192kbps (Standard)' },
  { value: 'mp3_128', label: 'MP3 128kbps (Compact)' },
  { value: 'flac', label: 'FLAC (Lossless)' },
];

const parseFormatValue = (value: string): RecordingFormat => {
  switch (value) {
    case 'wav':
      return { wav: {} };
    case 'mp3_320':
      return { mp3: { bitrate: 320 } };
    case 'mp3_192':
      return { mp3: { bitrate: 192 } };
    case 'mp3_128':
      return { mp3: { bitrate: 128 } };
    case 'flac':
      return { flac: { compression_level: 5 } };
    default:
      return { mp3: { bitrate: 192 } };
  }
};

const formatToValue = (format: RecordingFormat): string => {
  if (format.wav) return 'wav';
  if (format.mp3) {
    if (format.mp3.bitrate === 320) return 'mp3_320';
    if (format.mp3.bitrate === 128) return 'mp3_128';
    return 'mp3_192';
  }
  if (format.flac) return 'flac';
  return 'mp3_192';
};

export const RecordingControlsCard = memo<RecordingControlsCardProps>(({ disabled = false }) => {
  const { classes } = useStyles();
  const { status, actions, isLoading, error } = useRecording();

  // Quick recording config state
  const [quickConfig, setQuickConfig] = useState<Partial<RecordingConfig>>({
    name: 'Quick Recording',
    format: { mp3: { bitrate: 192 } },
    filename_template: '',
    metadata: {
      title: '',
      artist: '',
    },
    auto_stop_on_silence: false,
    silence_threshold_db: -60.0,
    silence_duration_sec: 5.0,
    sample_rate: 48000,
    channels: 2,
    bit_depth: 16, // Changed to 16-bit for better QuickTime compatibility
  });

  const [showAdvanced, setShowAdvanced] = useState(false);

  const isRecording = status?.is_recording ?? false;
  const currentSession = status?.current_session;
  const availableSpace = status?.available_space_gb ?? 0;

  // Load default config on first render (only non-text fields)
  useEffect(() => {
    const loadDefaultConfig = async () => {
      try {
        const defaultConfig = await actions.createDefaultConfig();
        setQuickConfig((prev) => ({
          ...defaultConfig,
          name: 'Quick Recording',
          // Keep user's text inputs and format selection, only load technical defaults
          format: prev.format || defaultConfig.format,
          filename_template: prev.filename_template ?? '',
          output_directory: prev.output_directory ?? defaultConfig.output_directory ?? '',
          metadata: {
            title: prev.metadata?.title ?? '',
            artist: prev.metadata?.artist ?? '',
          },
        }));
      } catch (err) {
        console.error('Failed to load default config:', err);
      }
    };

    void loadDefaultConfig();
  }, []);

  const handleStartRecording = useCallback(async () => {
    try {
      // Create full config from quick config
      const fullConfig: RecordingConfig = {
        id: crypto.randomUUID(),
        name: quickConfig.name || 'Quick Recording',
        format: quickConfig.format || { mp3: { bitrate: 192 } },
        output_directory: quickConfig.output_directory || '',
        filename_template: quickConfig.filename_template || '{timestamp}_{title}',
        metadata: quickConfig.metadata || {},
        auto_stop_on_silence: quickConfig.auto_stop_on_silence || false,
        silence_threshold_db: quickConfig.silence_threshold_db || -60.0,
        silence_duration_sec: quickConfig.silence_duration_sec || 5.0,
        max_duration_minutes: quickConfig.max_duration_minutes,
        max_file_size_mb: quickConfig.max_file_size_mb,
        split_on_interval_minutes: quickConfig.split_on_interval_minutes,
        sample_rate: quickConfig.sample_rate || 48000,
        channels: quickConfig.channels || 2,
        bit_depth: quickConfig.bit_depth || 16, // Use 16-bit for QuickTime/iTunes compatibility
      };

      console.log('Starting recording with config:', fullConfig);
      await actions.startRecording(fullConfig);
      console.log('Recording started successfully');
    } catch (err) {
      console.error('Failed to start recording:', err);
      alert(`Failed to start recording: ${err}`);
    }
  }, [quickConfig, actions]);

  const handleStopRecording = useCallback(async () => {
    try {
      await actions.stopRecording();
    } catch (err) {
      console.error('Failed to stop recording:', err);
    }
  }, [actions]);

  const handleDirectorySelect = useCallback(async () => {
    console.log('Browse button clicked');
    try {
      console.log('Calling select_recording_directory...');
      const selectedPath = await invoke<string | null>('select_recording_directory');
      console.log('Received path:', selectedPath);
      console.log('Current output directory before update:', quickConfig.output_directory);

      if (selectedPath) {
        console.log('Setting output directory to:', selectedPath);
        setQuickConfig((prev) => {
          const newConfig = {
            ...prev,
            output_directory: selectedPath,
          };
          console.log('New config after update:', newConfig);
          return newConfig;
        });
      } else {
        console.log('User cancelled folder selection');
      }
    } catch (err) {
      console.error('Failed to select directory:', err);
    }
  }, [quickConfig.output_directory]);

  const handleFormatChange = useCallback((value: string | null) => {
    if (value) {
      setQuickConfig((prev) => ({
        ...prev,
        format: parseFormatValue(value),
      }));
    }
  }, []);

  if (isLoading && !status) {
    return (
      <Card className={classes.recordingCard} padding="lg" withBorder>
        <Text c="dimmed">Loading recording service...</Text>
      </Card>
    );
  }

  return (
    <Card className={classes.recordingCard} padding="lg" withBorder>
      <Stack gap="md">
        <Group justify="space-between" align="center">
          <Group gap="xs">
            <IconFileMusic size={20} color="#fa5252" />
            <Title order={4} c="red.4">
              Audio Recording
            </Title>
          </Group>

          <Group className={classes.statusDisplay}>
            <Badge
              color={isRecording ? 'red' : 'gray'}
              variant={isRecording ? 'filled' : 'light'}
              size="md"
            >
              {isRecording ? '● REC' : 'Ready'}
            </Badge>
            {availableSpace < 1.0 && (
              <Tooltip label="Low disk space" position="top" withArrow>
                <IconAlertCircle size={16} color="#fa5252" />
              </Tooltip>
            )}
          </Group>
        </Group>

        {error && (
          <Alert icon={<IconAlertCircle size={16} />} title="Error" color="red" variant="light">
            {error}
          </Alert>
        )}

        {/* Recording Status Display */}
        {isRecording && currentSession && (
          <Stack gap="xs">
            <Group justify="space-between">
              <Text size="sm" c="dimmed">
                Recording: {currentSession.config.metadata.title || 'Untitled'}
              </Text>
              <Text size="sm" c="red.4" fw={600}>
                {formatDuration(currentSession.duration_seconds)}
              </Text>
            </Group>

            <Progress
              value={((currentSession.duration_seconds % 60) / 60) * 100}
              color="red"
              size="sm"
              className={classes.progressBar}
            />

            <Group justify="space-between">
              <Text size="xs" c="dimmed">
                Size: {formatFileSize(currentSession.file_size_bytes)}
              </Text>
              <Text size="xs" c="dimmed">
                Levels: L:{(currentSession.current_levels[0] * 100).toFixed(0)}% R:
                {(currentSession.current_levels[1] * 100).toFixed(0)}%
              </Text>
            </Group>
          </Stack>
        )}

        {/* Recording Controls */}
        <Group grow>
          {!isRecording ? (
            <Button
              onClick={handleStartRecording}
              leftSection={<IconCircleFilled size={20} />}
              disabled={disabled}
              color="red"
              size="lg"
              className={classes.recordingButton}
            >
              Start Recording
            </Button>
          ) : (
            <Button
              onClick={handleStopRecording}
              leftSection={<IconPlayerStop size={20} />}
              color="gray"
              size="lg"
              className={classes.recordingButton}
            >
              Stop Recording
            </Button>
          )}
        </Group>

        {/* Quick Configuration */}
        {!isRecording && (
          <div className={classes.configSection}>
            <Stack gap="sm">
              <Group justify="space-between" align="center">
                <Text size="sm" fw={500} c="gray.3">
                  Quick Settings
                </Text>
                <Button
                  variant="subtle"
                  size="xs"
                  leftSection={<IconSettings size={14} />}
                  onClick={() => setShowAdvanced(!showAdvanced)}
                >
                  {showAdvanced ? 'Simple' : 'Advanced'}
                </Button>
              </Group>

              {/* Essential Recording Fields */}
              <Group grow>
                <TextInput
                  label="Title"
                  placeholder="Enter recording title"
                  value={quickConfig.metadata?.title || ''}
                  onChange={(e) =>
                    setQuickConfig((prev) => ({
                      ...prev,
                      metadata: {
                        ...prev.metadata,
                        title: e.target.value,
                      },
                    }))
                  }
                  size="sm"
                  styles={{
                    label: { color: '#C1C2C5' },
                    input: { backgroundColor: '#2C2E33', borderColor: '#373A40', color: '#C1C2C5' },
                  }}
                />
                <TextInput
                  label="Artist"
                  placeholder="Enter artist name"
                  value={quickConfig.metadata?.artist || ''}
                  onChange={(e) =>
                    setQuickConfig((prev) => ({
                      ...prev,
                      metadata: {
                        ...prev.metadata,
                        artist: e.target.value,
                      },
                    }))
                  }
                  size="sm"
                  styles={{
                    label: { color: '#C1C2C5' },
                    input: { backgroundColor: '#2C2E33', borderColor: '#373A40', color: '#C1C2C5' },
                  }}
                />
              </Group>

              <Stack gap="xs">
                <Text size="sm" fw={500} c="#C1C2C5">
                  Output Directory
                </Text>
                <Group gap="sm">
                  <Text size="sm" c="#909296" style={{ flex: 1, minWidth: 0 }}>
                    {quickConfig.output_directory || 'Default (~/Music)'}
                  </Text>
                  <Button
                    size="sm"
                    variant="outline"
                    leftSection={<IconFolder size={16} />}
                    onClick={handleDirectorySelect}
                  >
                    Browse
                  </Button>
                </Group>
              </Stack>

              <Group grow>
                <Select
                  label="Format"
                  value={formatToValue(quickConfig.format || { mp3: { bitrate: 192 } })}
                  onChange={handleFormatChange}
                  data={getRecordingFormatOptions()}
                  size="sm"
                  styles={{
                    label: { color: '#C1C2C5' },
                    input: { backgroundColor: '#2C2E33', borderColor: '#373A40', color: '#C1C2C5' },
                  }}
                />
                <TextInput
                  label="Filename"
                  placeholder="Enter filename (without extension)"
                  value={quickConfig.filename_template || ''}
                  onChange={(e) =>
                    setQuickConfig((prev) => ({
                      ...prev,
                      filename_template: e.target.value,
                    }))
                  }
                  size="sm"
                  styles={{
                    label: { color: '#C1C2C5' },
                    input: { backgroundColor: '#2C2E33', borderColor: '#373A40', color: '#C1C2C5' },
                  }}
                />
              </Group>

              {showAdvanced && (
                <Stack gap="sm">
                  <Switch
                    label="Auto-stop on silence"
                    description="Stop recording after 5 seconds of silence"
                    checked={quickConfig.auto_stop_on_silence || false}
                    onChange={(event) =>
                      setQuickConfig((prev) => ({
                        ...prev,
                        auto_stop_on_silence: event.currentTarget.checked,
                      }))
                    }
                    size="sm"
                    styles={{
                      label: { color: '#C1C2C5' },
                      description: { color: '#909296' },
                    }}
                  />

                  <Group grow>
                    <NumberInput
                      label="Max Duration (minutes)"
                      placeholder="No limit"
                      value={quickConfig.max_duration_minutes || ''}
                      onChange={(value) =>
                        setQuickConfig((prev) => ({
                          ...prev,
                          max_duration_minutes: typeof value === 'number' ? value : undefined,
                        }))
                      }
                      min={1}
                      max={1440} // 24 hours
                      size="sm"
                      styles={{
                        label: { color: '#C1C2C5' },
                        input: {
                          backgroundColor: '#2C2E33',
                          borderColor: '#373A40',
                          color: '#C1C2C5',
                        },
                      }}
                    />
                    <NumberInput
                      label="Max File Size (MB)"
                      placeholder="No limit"
                      value={quickConfig.max_file_size_mb || ''}
                      onChange={(value) =>
                        setQuickConfig((prev) => ({
                          ...prev,
                          max_file_size_mb: typeof value === 'number' ? value : undefined,
                        }))
                      }
                      min={1}
                      max={10000}
                      size="sm"
                      styles={{
                        label: { color: '#C1C2C5' },
                        input: {
                          backgroundColor: '#2C2E33',
                          borderColor: '#373A40',
                          color: '#C1C2C5',
                        },
                      }}
                    />
                  </Group>
                </Stack>
              )}
            </Stack>
          </div>
        )}

        {/* Storage Info */}
        <Group justify="space-between">
          <Text size="xs" c="dimmed">
            Available: {availableSpace.toFixed(1)} GB
          </Text>
          <Text size="xs" c="dimmed">
            Total Recordings: {status?.total_recordings || 0}
          </Text>
        </Group>
      </Stack>
    </Card>
  );
});

RecordingControlsCard.displayName = 'RecordingControlsCard';
