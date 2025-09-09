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
  Modal,
  Divider,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconCircleFilled,
  IconPlayerPause,
  IconX,
  IconDeviceFloppy,
  IconSettings,
  IconFileMusic,
  IconAlertCircle,
  IconFolder,
} from '@tabler/icons-react';
import { invoke } from '@tauri-apps/api/core';
import { memo, useState, useCallback, useEffect } from 'react';

import { useRecording } from '../../hooks/use-recording';
import { DEFAULT_SAMPLE_RATE_HZ } from '../../utils';

import { MetadataForm } from './MetadataForm';

import type {
  RecordingConfig,
  RecordingFormat,
  RecordingMetadata,
} from '../../hooks/use-recording';

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
    format: { mp3: { bitrate: 192 } }, // MP3 encoder now properly implemented
    filename_template: '',
    metadata: {
      title: '',
      artist: '',
    },
    auto_stop_on_silence: false,
    silence_threshold_db: -60.0,
    silence_duration_sec: 5.0,
    sample_rate: DEFAULT_SAMPLE_RATE_HZ,
    channels: 2,
    bit_depth: 16, // Changed to 16-bit for better QuickTime compatibility
  });

  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showSaveModal, setShowSaveModal] = useState(false);
  const [showCancelModal, setShowCancelModal] = useState(false);
  const [finalMetadata, setFinalMetadata] = useState<RecordingMetadata | null>(null);
  const [finalFilePath, setFinalFilePath] = useState<string>('');

  const isRecording = status?.is_recording ?? false;
  const currentSession = status?.current_session; // Fixed to match backend field name
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
          format: prev.format ?? defaultConfig.format,
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
  }, [actions]);

  const handleStartRecording = useCallback(async () => {
    try {
      // Create full config from quick config
      const fullConfig: RecordingConfig = {
        id: crypto.randomUUID(),
        name: quickConfig.name ?? 'Quick Recording',
        format: quickConfig.format ?? { mp3: { bitrate: 192 } },
        output_directory: quickConfig.output_directory ?? '',
        filename_template: quickConfig.filename_template ?? '{timestamp}_{title}',
        metadata: quickConfig.metadata ?? {},
        auto_stop_on_silence: quickConfig.auto_stop_on_silence ?? false,
        silence_threshold_db: quickConfig.silence_threshold_db ?? -60.0,
        silence_duration_sec: quickConfig.silence_duration_sec ?? 5.0,
        max_duration_minutes: quickConfig.max_duration_minutes,
        max_file_size_mb: quickConfig.max_file_size_mb,
        split_on_interval_minutes: quickConfig.split_on_interval_minutes,
        sample_rate: quickConfig.sample_rate ?? DEFAULT_SAMPLE_RATE_HZ,
        channels: quickConfig.channels ?? 2,
        bit_depth: quickConfig.bit_depth ?? 16, // Use 16-bit for QuickTime/iTunes compatibility
      };

      console.log('Starting recording with config:', fullConfig);
      await actions.startRecording(fullConfig);
      console.log('Recording started successfully');
    } catch (err) {
      console.error('Failed to start recording:', err);
      alert(`Failed to start recording: ${err}`);
    }
  }, [quickConfig, actions]);

  // Removed handleStopRecording - using Cancel/Save instead

  const handlePauseRecording = useCallback(async () => {
    try {
      // TODO: Implement pause functionality in backend
      console.log('Pause recording - not yet implemented in backend');
    } catch (err) {
      console.error('Failed to pause recording:', err);
    }
  }, []);

  const handleCancelRecording = useCallback(() => {
    // Show confirmation modal instead of immediately cancelling
    setShowCancelModal(true);
  }, []);

  const handleConfirmCancel = useCallback(async () => {
    try {
      // Stop recording and discard the file
      const result = await actions.stopRecording();
      console.log('Recording cancelled and file discarded:', result);
      setShowCancelModal(false);
      // TODO: Delete the temporary file from filesystem
    } catch (err) {
      console.error('Failed to cancel recording:', err);
      setShowCancelModal(false);
    }
  }, [actions]);

  const handleSaveRecording = useCallback(async () => {
    try {
      // Stop recording and prepare for metadata editing
      const result = await actions.stopRecording();
      if (result && currentSession) {
        setFinalMetadata(currentSession.metadata || {});
        setFinalFilePath(currentSession.current_file_path || '');
        setShowSaveModal(true);
      }
    } catch (err) {
      console.error('Failed to save recording:', err);
    }
  }, [actions, currentSession]);

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

  return (
    <>
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
          {!isRecording ? (
            <Group grow>
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
            </Group>
          ) : (
            <Group grow>
              <Button
                onClick={handlePauseRecording}
                leftSection={<IconPlayerPause size={20} />}
                color="yellow"
                variant="light"
                size="md"
              >
                Pause
              </Button>
              <Button
                onClick={handleCancelRecording}
                leftSection={<IconX size={20} />}
                color="gray"
                variant="outline"
                size="md"
              >
                Cancel
              </Button>
              <Button
                onClick={handleSaveRecording}
                leftSection={<IconDeviceFloppy size={20} />}
                color="green"
                size="md"
              >
                Save
              </Button>
            </Group>
          )}

          {/* Configuration - Show settings and metadata forms */}
          <div className={classes.configSection}>
            <Stack gap="sm">
              <Group justify="space-between" align="center">
                <Text size="sm" fw={500} c="gray.3">
                  {isRecording ? 'Recording Settings' : 'Quick Settings'}
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

              {/* Metadata Form - Show when Advanced is enabled */}
              {showAdvanced && (
                <div
                  style={{
                    backgroundColor: '#25262B',
                    padding: '16px',
                    borderRadius: '8px',
                    border: '1px solid #373A40',
                    marginTop: '8px',
                  }}
                >
                  <Text size="sm" fw={500} c="#C1C2C5" mb="md">
                    Recording Metadata
                  </Text>
                  <MetadataForm
                    metadata={quickConfig.metadata || {}}
                    onChange={(metadata) =>
                      setQuickConfig((prev) => ({
                        ...prev,
                        metadata,
                      }))
                    }
                    showPresetButtons={false}
                  />
                </div>
              )}

              {/* Output Directory - Only show when not recording */}
              {!isRecording && (
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
              )}

              {/* Format and Filename - Only show when not recording */}
              {!isRecording && (
                <Group grow>
                  <Select
                    label="Format"
                    value={formatToValue(quickConfig.format || { mp3: { bitrate: 192 } })}
                    onChange={handleFormatChange}
                    data={getRecordingFormatOptions()}
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
                      input: {
                        backgroundColor: '#2C2E33',
                        borderColor: '#373A40',
                        color: '#C1C2C5',
                      },
                    }}
                  />
                </Group>
              )}

              {/* Advanced Recording Settings - Available during recording */}
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

      {/* Cancel Confirmation Modal */}
      <Modal
        opened={showCancelModal}
        onClose={() => setShowCancelModal(false)}
        title={
          <Text fw={600} c="red">
            Cancel Recording
          </Text>
        }
        centered
        size="sm"
      >
        <Stack gap="md">
          <Text>Are you sure you want to cancel this recording?</Text>
          <Text c="red" size="sm">
            This cannot be reversed and all recorded audio will be lost.
          </Text>
          <Group justify="flex-end" gap="sm">
            <Button variant="subtle" onClick={() => setShowCancelModal(false)}>
              Keep Recording
            </Button>
            <Button color="red" onClick={handleConfirmCancel} leftSection={<IconX size={16} />}>
              Cancel Recording
            </Button>
          </Group>
        </Stack>
      </Modal>

      {/* Save Recording Modal with Full Metadata Form */}
      <Modal
        opened={showSaveModal}
        onClose={() => setShowSaveModal(false)}
        title={
          <Text fw={600} c="green">
            Save Recording
          </Text>
        }
        size="xl"
        centered
      >
        <Stack gap="md">
          <Text>Complete the metadata and file information before saving your recording.</Text>

          <Divider />

          {/* File Path Section */}
          <Stack gap="xs">
            <Text size="sm" fw={500}>
              Output Location
            </Text>
            <Group gap="sm">
              <TextInput
                label="File Path"
                value={finalFilePath}
                onChange={(e) => setFinalFilePath(e.target.value)}
                placeholder="Choose where to save your recording"
                style={{ flex: 1 }}
                rightSection={
                  <Button size="xs" variant="subtle">
                    Browse
                  </Button>
                }
              />
            </Group>
          </Stack>

          <Divider />

          {/* Complete Metadata Form */}
          {finalMetadata && (
            <MetadataForm metadata={finalMetadata} onChange={setFinalMetadata} showPresetButtons />
          )}

          <Group justify="flex-end" gap="sm" mt="md">
            <Button variant="subtle" onClick={() => setShowSaveModal(false)}>
              Cancel
            </Button>
            <Button
              color="green"
              onClick={() => {
                // TODO: Implement final save with metadata and file path
                console.log('Saving with metadata:', finalMetadata);
                console.log('Saving to path:', finalFilePath);
                setShowSaveModal(false);
              }}
              leftSection={<IconDeviceFloppy size={16} />}
            >
              Save Recording
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
});

RecordingControlsCard.displayName = 'RecordingControlsCard';
