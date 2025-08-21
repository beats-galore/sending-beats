import { Card, Title, Group, Box, Text, Stack, Badge, Center, ThemeIcon } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconMusic } from '@tabler/icons-react';
import { memo } from 'react';

type StreamMetadata = {
  title: string;
  artist: string;
  album?: string;
  genre?: string;
};

type NowPlayingCardProps = {
  currentMetadata: StreamMetadata | null;
};

const useStyles = createStyles((theme) => ({
  nowPlayingCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
    minHeight: 200,
  },

  musicIcon: {
    width: 80,
    height: 80,
    backgroundColor: theme.colors.blue[6],
    borderRadius: theme.radius.lg,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: '2rem',
  },

  trackInfo: {
    flex: 1,
  },

  pulseIndicator: {
    width: 8,
    height: 8,
    backgroundColor: theme.colors.red[6],
    borderRadius: '50%',
    animation: 'pulse 2s infinite',
  },
}));

export const NowPlayingCard = memo<NowPlayingCardProps>(({ currentMetadata }) => {
  const { classes } = useStyles();

  return (
    <Card className={classes.nowPlayingCard} padding="lg" withBorder>
      <Title order={3} c="blue.4" mb="lg">
        Now Playing
      </Title>

      {currentMetadata ? (
        <Group gap="lg" align="center">
          <Box className={classes.musicIcon}>🎵</Box>
          <Box className={classes.trackInfo}>
            <Text size="xl" fw={700} c="white" mb={4}>
              {currentMetadata.title}
            </Text>
            <Text size="md" c="orange.4" mb={2}>
              {currentMetadata.artist}
            </Text>
            {currentMetadata.album && (
              <Text size="sm" c="dimmed">
                {currentMetadata.album}
              </Text>
            )}
          </Box>
          <Stack align="center" gap={4}>
            <Badge color="red" variant="filled" size="xs">
              LIVE
            </Badge>
            <Box className={classes.pulseIndicator} />
          </Stack>
        </Group>
      ) : (
        <Center py="xl">
          <Stack align="center" gap="md">
            <ThemeIcon size={60} color="blue" variant="light">
              <IconMusic size={30} />
            </ThemeIcon>
            <Stack align="center" gap={4}>
              <Text size="lg" fw={600}>
                No track information available
              </Text>
              <Text size="sm" c="dimmed" ta="center">
                Track metadata will appear here when available
              </Text>
            </Stack>
          </Stack>
        </Center>
      )}
    </Card>
  );
});

NowPlayingCard.displayName = 'NowPlayingCard';
