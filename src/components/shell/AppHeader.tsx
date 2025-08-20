import { AppShell, Group, Title, Badge, Burger, ActionIcon } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';
import { IconMenu2, IconMenuDeep } from '@tabler/icons-react';

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

export const AppHeader = memo<AppHeaderProps>(({ 
  mobileOpened, 
  desktopOpened, 
  toggleMobile, 
  toggleDesktop, 
  currentView 
}) => {
  const { classes } = useStyles();
  
  return (
    <AppShell.Header className={classes.header}>
      <Group className={classes.headerContent}>
        <Group>
          <Burger
            opened={mobileOpened}
            onClick={toggleMobile}
            hiddenFrom="sm"
            size="sm"
          />
          <ActionIcon
            onClick={toggleDesktop}
            variant="subtle"
            visibleFrom="sm"
            size="sm"
            title={desktopOpened ? "Collapse sidebar" : "Expand sidebar"}
          >
            {desktopOpened ? <IconMenuDeep size={18} /> : <IconMenu2 size={18} />}
          </ActionIcon>
          <Title order={2} className={classes.logo}>
            Sendin Beats Radio
          </Title>
        </Group>
        
        <Badge
          color={currentView === 'mixer' ? 'green' : 'gray'}
          variant="light"
          size="sm"
        >
          {currentView === 'mixer' ? 'Mixer Active' : 'Studio Offline'}
        </Badge>
      </Group>
    </AppShell.Header>
  );
});

AppHeader.displayName = 'AppHeader';