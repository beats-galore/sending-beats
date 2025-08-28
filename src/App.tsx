// Professional Radio Streaming Platform - Modernized with Mantine
import { AppShell, Box } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { createStyles } from '@mantine/styles';
import { memo, useCallback, useState } from 'react';

import AdminPanel from './components/AdminPanel';
import DJClient from './components/DJClient';
import { ErrorBoundary } from './components/layout';
import ListenerPlayer from './components/ListenerPlayer';
import { VirtualMixerWithErrorBoundary as VirtualMixer } from './components/mixer';
import { AppHeader, AppFooter, Navigation, HomeView } from './components/shell';
import { PermissionModal } from './components/ui/PermissionModal';
import { useStartupPermissionCheck } from './hooks/useStartupPermissionCheck';

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
    height: 'calc(100vh - 100px)', // Account for header + footer
    overflow: 'hidden', // Prevent main from scrolling
  },

  content: {
    height: '100%',
    width: '100%',
    maxWidth: '100%',
    overflow: 'hidden', // Let child components handle their own scrolling
    display: 'flex',
    flexDirection: 'column',
  },
}));

const App = memo(() => {
  const { classes } = useStyles();
  const [mobileOpened, { toggle: toggleMobile }] = useDisclosure(false);
  const [desktopOpened, { toggle: toggleDesktop }] = useDisclosure(false);
  const [currentView, setCurrentView] = useState<ViewType>('mixer');
  
  // Permission check for application audio capture
  const {
    showPermissionModal,
    handleCloseModal,
    handleOpenSystemPreferences,
    isLoading: permissionLoading,
  } = useStartupPermissionCheck();

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
          width: desktopOpened ? 250 : 80,
          breakpoint: 'sm',
          collapsed: { mobile: !mobileOpened, desktop: false },
        }}
        footer={{ height: 40 }}
        padding={{ base: 'xs', sm: 'sm', md: 'md' }}
      >
        <AppHeader
          mobileOpened={mobileOpened}
          desktopOpened={desktopOpened}
          toggleMobile={toggleMobile}
          toggleDesktop={toggleDesktop}
          currentView={currentView}
        />

        <AppShell.Navbar className={classes.navbar} p="md">
          <Navigation
            currentView={currentView}
            onViewChange={handleViewChange}
            collapsed={!desktopOpened}
          />
        </AppShell.Navbar>

        <AppShell.Main className={classes.main}>
          <Box className={classes.content}>{renderContent()}</Box>
        </AppShell.Main>

        <AppFooter />
      </AppShell>
      
      {/* Permission Modal for Application Audio Capture */}
      <PermissionModal
        isOpen={showPermissionModal}
        onClose={handleCloseModal}
        onOpenSystemPreferences={handleOpenSystemPreferences}
        isLoading={permissionLoading}
      />
    </ErrorBoundary>
  );
});

App.displayName = 'App';

export default App;
