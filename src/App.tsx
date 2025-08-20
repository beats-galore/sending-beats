// Professional Radio Streaming Platform - Modernized with Mantine
import { AppShell, Box } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { createStyles } from '@mantine/styles';
import { memo, useCallback, useState } from 'react';

import { ErrorBoundary } from './components/layout';
import { AppHeader, AppFooter, Navigation, HomeView } from './components/shell';
import AdminPanel from './components/AdminPanel';
import DJClient from './components/DJClient';
import ListenerPlayer from './components/ListenerPlayer';
import { VirtualMixerWithErrorBoundary as VirtualMixer } from './components/mixer';

type ViewType = 'home' | 'dj' | 'admin' | 'listener' | 'mixer';

const useStyles = createStyles((theme) => ({
  appShell: {
    backgroundColor: theme.colors.dark[8],
  },
  
  navbar: {
    backgroundColor: theme.colors.dark[7],
    borderRight: `1px solid ${theme.colors.dark[5]}`,
  },
  
  main: {
    backgroundColor: theme.colors.dark[8],
    minHeight: '100vh',
  },
  
  content: {
    height: '100%',
  },
}));

const App = memo(() => {
  const { classes } = useStyles();
  const [opened, { toggle }] = useDisclosure();
  const [currentView, setCurrentView] = useState<ViewType>('mixer');

  console.log('re-rendered app');

  const renderContent = useCallback(() => {
    switch (currentView) {
      case 'mixer':
        return <VirtualMixer />;
      case 'dj':
        return <DJClient />;
      case 'admin':
        return <AdminPanel />;
      case 'listener':
        return <ListenerPlayer />;
      case 'home':
      default:
        return <HomeView />;
    }
  }, [currentView]);

  const handleViewChange = useCallback((view: ViewType) => {
    setCurrentView(view);
  }, []);

  return (
    <ErrorBoundary>
      <AppShell
        className={classes.appShell}
        header={{ height: 60 }}
        navbar={{
          width: 250,
          breakpoint: 'sm',
          collapsed: { mobile: !opened },
        }}
        footer={{ height: 40 }}
        padding="md"
      >
        <AppHeader
          opened={opened}
          toggle={toggle}
          currentView={currentView}
        />

        <AppShell.Navbar className={classes.navbar} p="md">
          <Navigation
            currentView={currentView}
            onViewChange={handleViewChange}
          />
        </AppShell.Navbar>

        <AppShell.Main className={classes.main}>
          <Box className={classes.content}>
            {renderContent()}
          </Box>
        </AppShell.Main>

        <AppFooter />
      </AppShell>
    </ErrorBoundary>
  );
});

App.displayName = 'App';

export default App;
