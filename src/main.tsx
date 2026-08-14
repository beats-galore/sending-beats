import { MantineProvider } from '@mantine/core';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { studioCssVariablesResolver, studioTheme } from './theme/theme';

import '@mantine/core/styles.css';

// React Scan setup for development performance monitoring
if (typeof window !== 'undefined' && import.meta.env.REACT_SCAN_ENABLED === 'true') {
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
