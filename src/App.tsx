import { memo } from 'react';

import { ErrorBoundary } from './components/layout';
import { StudioShell } from './components/studio/StudioShell';
import { PermissionModal } from './components/ui/PermissionModal';
import { useStartupPermissionCheck } from './hooks/use-startup-permission-check';

export const App = memo(() => {
  const {
    showPermissionModal,
    handleCloseModal,
    handleOpenSystemPreferences,
    isLoading: permissionLoading,
  } = useStartupPermissionCheck();

  return (
    <ErrorBoundary>
      <StudioShell />

      <PermissionModal
        isOpen={showPermissionModal}
        onClose={handleCloseModal}
        onOpenSystemPreferences={() => void handleOpenSystemPreferences()}
        isLoading={permissionLoading}
      />
    </ErrorBoundary>
  );
});

App.displayName = 'App';
