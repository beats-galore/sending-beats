import { Card, Stack, Group, Title, Text, Button, Badge, ScrollArea, ActionIcon, Tooltip, Modal } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconHistory, IconPlayerPlay, IconFolder, IconDownload, IconMusic } from '@tabler/icons-react';
import { memo, useState, useCallback } from 'react';

import { useRecording } from '../../hooks/use-recording';

import type { RecordingHistoryEntry } from '../../hooks/use-recording';

type RecordingHistoryCardProps = {
  disabled?: boolean;
  maxHeight?: number;
};

const useStyles = createStyles((theme) => ({
  historyCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },
  
  historyItem: {
    backgroundColor: theme.colors.dark[7],
    padding: theme.spacing.sm,
    borderRadius: theme.radius.md,
    border: `1px solid ${theme.colors.dark[5]}`,
    transition: 'border-color 0.2s ease',
    
    '&:hover': {
      borderColor: theme.colors.dark[3],
    },
  },
  
  emptyState: {
    textAlign: 'center',
    color: theme.colors.gray[5],
    padding: theme.spacing.xl,
  },
  
  actionButton: {
    '&:hover': {
      backgroundColor: theme.colors.dark[5],
    },
  },
  
  scrollArea: {
    maxHeight: '400px',
  },
  
  fileInfo: {
    fontFamily: theme.fontFamilyMonospace,
    fontSize: theme.fontSizes.xs,
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
  if (bytes === 0) {return '0 B';}
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / k**i).toFixed(1))} ${sizes[i]}`;
};

const formatDate = (dateString: string): string => {
  try {
    const date = new Date(dateString);
    return `${date.toLocaleDateString()} ${date.toLocaleTimeString([], { 
      hour: '2-digit', 
      minute: '2-digit' 
    })}`;
  } catch {
    return 'Unknown date';
  }
};

const getFileFormat = (filePath: string): string => {
  const extension = filePath.split('.').pop()?.toLowerCase();
  switch (extension) {
    case 'mp3':
      return 'MP3';
    case 'wav':
      return 'WAV';
    case 'flac':
      return 'FLAC';
    default:
      return extension?.toUpperCase() || 'Unknown';
  }
};

const getFormatColor = (format: string): string => {
  switch (format) {
    case 'MP3':
      return 'blue';
    case 'WAV':
      return 'green';
    case 'FLAC':
      return 'violet';
    default:
      return 'gray';
  }
};

export const RecordingHistoryCard = memo<RecordingHistoryCardProps>(
  ({ disabled = false, maxHeight = 400 }) => {
    const { classes } = useStyles();
    const { history, actions } = useRecording();
    
    const [selectedEntry, setSelectedEntry] = useState<RecordingHistoryEntry | null>(null);
    const [detailsModalOpened, setDetailsModalOpened] = useState(false);
    
    const handleShowDetails = useCallback((entry: RecordingHistoryEntry) => {
      setSelectedEntry(entry);
      setDetailsModalOpened(true);
    }, []);
    
    const handleOpenFolder = useCallback((filePath: string) => {
      // This would open the file location in the system file manager
      // For now, just log the path - would need platform-specific implementation
      console.log('Open folder:', filePath);
    }, []);
    
    const handlePlayFile = useCallback((filePath: string) => {
      // This would play the audio file
      // For now, just log the path - would need audio player implementation
      console.log('Play file:', filePath);
    }, []);
    
    const sortedHistory = [...history].sort((a, b) => 
      new Date(b.start_time).getTime() - new Date(a.start_time).getTime()
    );
    
    return (
      <>
        <Card className={classes.historyCard} padding="lg" withBorder>
          <Stack gap="md">
            <Group justify="space-between" align="center">
              <Group gap="xs">
                <IconHistory size={20} color="#68b0a6" />
                <Title order={4} c="teal.4">
                  Recording History
                </Title>
              </Group>
              
              <Badge variant="light" color="teal">
                {history.length} recordings
              </Badge>
            </Group>
            
            {sortedHistory.length === 0 ? (
              <div className={classes.emptyState}>
                <IconMusic size={48} color="gray" style={{ opacity: 0.3 }} />
                <Text size="sm" mt="md">
                  No recordings yet
                </Text>
                <Text size="xs" c="dimmed">
                  Start your first recording to see it here
                </Text>
              </div>
            ) : (
              <ScrollArea className={classes.scrollArea} style={{ maxHeight }}>
                <Stack gap="xs">
                  {sortedHistory.map((entry) => {
                    const format = getFileFormat(entry.file_path);
                    const fileName = entry.file_path.split('/').pop() || 'Unknown';
                    
                    return (
                      <div key={entry.id} className={classes.historyItem}>
                        <Stack gap="xs">
                          <Group justify="space-between" align="flex-start">
                            <Stack gap={2} style={{ flex: 1 }}>
                              <Group gap="xs" align="center">
                                <Text size="sm" fw={500} lineClamp={1}>
                                  {entry.metadata.title || fileName}
                                </Text>
                                <Badge 
                                  size="xs" 
                                  color={getFormatColor(format)}
                                  variant="light"
                                >
                                  {format}
                                </Badge>
                              </Group>
                              
                              {entry.metadata.artist && (
                                <Text size="xs" c="dimmed">
                                  by {entry.metadata.artist}
                                </Text>
                              )}
                              
                              <Group gap="md">
                                <Text size="xs" c="dimmed">
                                  {formatDate(entry.start_time)}
                                </Text>
                                <Text size="xs" c="dimmed">
                                  {formatDuration(entry.duration_seconds)}
                                </Text>
                                <Text size="xs" c="dimmed">
                                  {formatFileSize(entry.file_size_bytes)}
                                </Text>
                              </Group>
                            </Stack>
                            
                            <Group gap="xs">
                              <Tooltip label="Show details" position="top" withArrow>
                                <ActionIcon
                                  variant="subtle"
                                  size="sm"
                                  className={classes.actionButton}
                                  onClick={() => handleShowDetails(entry)}
                                  disabled={disabled}
                                >
                                  <IconDownload size={14} />
                                </ActionIcon>
                              </Tooltip>
                              
                              <Tooltip label="Open folder" position="top" withArrow>
                                <ActionIcon
                                  variant="subtle"
                                  size="sm"
                                  className={classes.actionButton}
                                  onClick={() => handleOpenFolder(entry.file_path)}
                                  disabled={disabled}
                                >
                                  <IconFolder size={14} />
                                </ActionIcon>
                              </Tooltip>
                              
                              <Tooltip label="Play" position="top" withArrow>
                                <ActionIcon
                                  variant="subtle"
                                  size="sm"
                                  className={classes.actionButton}
                                  onClick={() => handlePlayFile(entry.file_path)}
                                  disabled={disabled}
                                  color="green"
                                >
                                  <IconPlayerPlay size={14} />
                                </ActionIcon>
                              </Tooltip>
                            </Group>
                          </Group>
                        </Stack>
                      </div>
                    );
                  })}
                </Stack>
              </ScrollArea>
            )}
          </Stack>
        </Card>
        
        {/* Recording Details Modal */}
        <Modal
          opened={detailsModalOpened}
          onClose={() => setDetailsModalOpened(false)}
          title="Recording Details"
          size="md"
        >
          {selectedEntry && (
            <Stack gap="md">
              <Group>
                <Text fw={500}>Title:</Text>
                <Text>{selectedEntry.metadata.title || 'Untitled'}</Text>
              </Group>
              
              {selectedEntry.metadata.artist && (
                <Group>
                  <Text fw={500}>Artist:</Text>
                  <Text>{selectedEntry.metadata.artist}</Text>
                </Group>
              )}
              
              {selectedEntry.metadata.album && (
                <Group>
                  <Text fw={500}>Album:</Text>
                  <Text>{selectedEntry.metadata.album}</Text>
                </Group>
              )}
              
              {selectedEntry.metadata.genre && (
                <Group>
                  <Text fw={500}>Genre:</Text>
                  <Text>{selectedEntry.metadata.genre}</Text>
                </Group>
              )}
              
              <Group>
                <Text fw={500}>Duration:</Text>
                <Text>{formatDuration(selectedEntry.duration_seconds)}</Text>
              </Group>
              
              <Group>
                <Text fw={500}>File Size:</Text>
                <Text>{formatFileSize(selectedEntry.file_size_bytes)}</Text>
              </Group>
              
              <Group>
                <Text fw={500}>Format:</Text>
                <Text>{getFileFormat(selectedEntry.file_path)}</Text>
              </Group>
              
              <Group>
                <Text fw={500}>Sample Rate:</Text>
                <Text>{selectedEntry.metadata.sample_rate || 'Unknown'} Hz</Text>
              </Group>
              
              <Group>
                <Text fw={500}>Channels:</Text>
                <Text>2</Text>
              </Group>
              
              <Group>
                <Text fw={500}>Recorded:</Text>
                <Text>{formatDate(selectedEntry.start_time)}</Text>
              </Group>
              
              <Stack gap="xs">
                <Text fw={500}>File Location:</Text>
                <Text size="xs" className={classes.fileInfo} c="dimmed">
                  {selectedEntry.file_path}
                </Text>
              </Stack>
              
              {selectedEntry.metadata.comment && (
                <Stack gap="xs">
                  <Text fw={500}>Comment:</Text>
                  <Text size="sm" c="dimmed">
                    {selectedEntry.metadata.comment}
                  </Text>
                </Stack>
              )}
              
              <Group justify="flex-end" gap="sm" pt="md">
                <Button
                  variant="subtle"
                  leftSection={<IconFolder size={16} />}
                  onClick={() => handleOpenFolder(selectedEntry.file_path)}
                >
                  Open Folder
                </Button>
                <Button
                  leftSection={<IconPlayerPlay size={16} />}
                  onClick={() => handlePlayFile(selectedEntry.file_path)}
                  color="green"
                >
                  Play
                </Button>
              </Group>
            </Stack>
          )}
        </Modal>
      </>
    );
  }
);

RecordingHistoryCard.displayName = 'RecordingHistoryCard';