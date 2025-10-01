import { Center, Stack, Title, Text } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

import { NowPlayingCard } from './NowPlayingCard';

const useStyles = createStyles(() => ({
  homeContainer: {
    height: '100%',
  },

  welcomeText: {
    textAlign: 'center',
  },
}));

export const HomeView = memo(() => {
  const { classes } = useStyles();

  return (
    <Center h="100%">
      <Stack align="center" gap="xl">
        <NowPlayingCard />
        <Stack align="center" gap="md">
          <Title order={2} c="blue.4" ta="center">
            Welcome to Sendin Beats Radio
          </Title>
          <Text c="dimmed" ta="center">
            radio streaming platform for DJs and listeners
          </Text>
        </Stack>
      </Stack>
    </Center>
  );
});

HomeView.displayName = 'HomeView';
