import { Box, Card, Group, Stack, Text, Badge } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

const useStyles = createStyles((theme) => ({
  nowPlayingCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
    maxWidth: 400,
    margin: '0 auto',
  },

  musicIcon: {
    width: 60,
    height: 60,
    backgroundColor: theme.colors.blue[6],
    borderRadius: theme.radius.lg,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: '24px',
  },

  liveBadge: {
    backgroundColor: theme.colors.red[6],
    color: theme.white,
  },

  pulseIndicator: {
    width: 8,
    height: 8,
    backgroundColor: theme.colors.red[6],
    borderRadius: '50%',
    animation: 'pulse 2s infinite',
  },

  trackInfo: {
    flex: 1,
  },
}));

export const NowPlayingCard = memo(() => {
  const { classes } = useStyles();

  return (
    <Card className={classes.nowPlayingCard} padding="lg" shadow="md" withBorder>
      <Group gap="md">
        <Box className={classes.musicIcon}>🎵</Box>

        <Box className={classes.trackInfo}>
          <Text size="sm" c="blue.4" fw={600} mb={4}>
            Now Playing
          </Text>
          <Text size="lg" fw={700} c="white" mb={2}>
            Midnight Groove
          </Text>
          <Text size="sm" c="dimmed">
            DJ Luna
          </Text>
        </Box>

        <Stack align="center" gap={4}>
          <Badge className={classes.liveBadge} size="xs" variant="filled">
            LIVE
          </Badge>
          <Box className={classes.pulseIndicator} />
        </Stack>
      </Group>
    </Card>
  );
});

NowPlayingCard.displayName = 'NowPlayingCard';
