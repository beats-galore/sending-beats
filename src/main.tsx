import { MantineProvider } from '@mantine/core';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { studioCssVariablesResolver, studioTheme } from './theme/theme';

import '@mantine/core/styles.css';

// React Scan is opt-in: it only loads in a dev build and only when
// VITE_REACT_SCAN=true is set, so profiling never turns itself on.
if (
  typeof window !== 'undefined' &&
  import.meta.env.DEV &&
  import.meta.env.VITE_REACT_SCAN === 'true'
) {
  import('react-scan')
    .then((ReactScan) => {
      ReactScan.scan({
        enabled: true,
        log: true,
      });
    })
    .catch(() => {
      // React Scan is a dev dependency; skip it if it is not installed
    });
}

const container = document.getElementById('root');
if (container) {
  const root = createRoot(container);
  root.render(
    <MantineProvider
      theme={studioTheme}
      cssVariablesResolver={studioCssVariablesResolver}
      forceColorScheme="dark"
    >
      <App />
    </MantineProvider>
  );
}
