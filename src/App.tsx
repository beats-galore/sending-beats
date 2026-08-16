import { memo } from 'react';

import { ErrorBoundary } from './components/layout';
import { StudioShell } from './components/studio/StudioShell';

export const App = memo(() => (
  <ErrorBoundary>
    <StudioShell />
  </ErrorBoundary>
));

App.displayName = 'App';
