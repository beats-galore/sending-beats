import { AppShell, Group, Title, Badge, Burger, ActionIcon, Button } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconMenu2, IconMenuDeep, IconMicrophone } from '@tabler/icons-react';
import { invoke } from '@tauri-apps/api/core';
import { memo, useCallback } from 'react';

type ViewType = 'home' | 'dj' | 'admin' | 'listener' | 'mixer';

type AppHeaderProps = {
  mobileOpened: boolean;
  desktopOpened: boolean;
  toggleMobile: () => void;
  toggleDesktop: () => void;
  currentView: ViewType;
};

const useStyles = createStyles((theme) => ({
  header: {
    backgroundColor: theme.colors.dark[7],
    borderBottom: `1px solid ${theme.colors.dark[5]}`,
  },

  headerContent: {
    height: '100%',
    paddingLeft: theme.spacing.md,
    paddingRight: theme.spacing.md,
    justifyContent: 'space-between',
  },

  logo: {
    color: theme.colors.blue[4],
  },
}));

export const AppHeader = memo<AppHeaderProps>(
  ({ mobileOpened, desktopOpened, toggleMobile, toggleDesktop, currentView }) => {
    const { classes } = useStyles();

    // Development helper to trigger permission request
    const handleRequestPermissions = useCallback(async () => {
      console.log('🔐 Requesting audio capture permissions...');
      try {
        const result = await invoke<string>('request_audio_capture_permissions');
        console.log('✅ Permission request result:', result);
        alert(`Permission Request Result:\n\n${result}`);
      } catch (error) {
        console.error('❌ Permission request failed:', error);
        alert(`Permission request failed: ${error}`);
      }
    }, []);

    return (
      <AppShell.Header className={classes.header}>
        <Group className={classes.headerContent}>
          <Group>
            <Burger opened={mobileOpened} onClick={toggleMobile} hiddenFrom="sm" size="sm" />
            <ActionIcon
              onClick={toggleDesktop}
              variant="subtle"
              visibleFrom="sm"
              size="sm"
              title={desktopOpened ? 'Collapse sidebar' : 'Expand sidebar'}
            >
              {desktopOpened ? <IconMenuDeep size={18} /> : <IconMenu2 size={18} />}
            </ActionIcon>
            <Title order={2} className={classes.logo}>
              Sendin Beats Radio
            </Title>
          </Group>

          <Group gap="xs">
            {/* Permission request button for development */}
            <Button
              size="xs"
              variant="light"
              color="orange"
              leftSection={<IconMicrophone size={14} />}
              onClick={handleRequestPermissions}
              title="Trigger permission request - will add app to System Preferences"
            >
              Request Permissions
            </Button>
            <Badge color={currentView === 'mixer' ? 'green' : 'gray'} variant="light" size="sm">
              {currentView === 'mixer' ? 'Mixer Active' : 'Studio Offline'}
            </Badge>
          </Group>
        </Group>
      </AppShell.Header>
    );
  }
);

AppHeader.displayName = 'AppHeader';
