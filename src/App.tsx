import { memo } from 'react';

import { ErrorBoundary } from './components/layout';
import { StudioShell } from './components/studio/StudioShell';
import { PermissionModal } from './components/ui/PermissionModal';
import { useStartupPermissionCheck } from './hooks/use-startup-permission-check';

// Kept separate from App so that everything, including the startup permission
// hook, runs inside the error boundary. Calling the hook in App itself put it
// above the boundary, where a throw takes down the whole tree and renders a
// blank page instead of the fallback.
const Studio = () => {
  const {
    showPermissionModal,
    handleCloseModal,
    handleOpenSystemPreferences,
    isLoading: permissionLoading,
  } = useStartupPermissionCheck();

  return (
    <>
      <StudioShell />

      <PermissionModal
        isOpen={showPermissionModal}
        onClose={handleCloseModal}
        onOpenSystemPreferences={() => void handleOpenSystemPreferences()}
        isLoading={permissionLoading}
      />
    </>
  );
};

export const App = memo(() => (
  <ErrorBoundary>
    <Studio />
  </ErrorBoundary>
));

App.displayName = 'App';
