import { AppShell, Group, Title, Badge, Burger } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

type ViewType = 'home' | 'dj' | 'admin' | 'listener' | 'mixer';

type AppHeaderProps = {
  opened: boolean;
  toggle: () => void;
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

export const AppHeader = memo<AppHeaderProps>(({ opened, toggle, currentView }) => {
  const { classes } = useStyles();
  
  return (
    <AppShell.Header className={classes.header}>
      <Group className={classes.headerContent}>
        <Group>
          <Burger
            opened={opened}
            onClick={toggle}
            hiddenFrom="sm"
            size="sm"
          />
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