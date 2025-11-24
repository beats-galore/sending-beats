import { MantineProvider } from '@mantine/core';
import { createRoot } from 'react-dom/client';

import '@mantine/core/styles.css';

// React Scan setup for development performance monitoring
if (typeof window !== 'undefined' && import.meta.env.REACT_SCAN_ENABLED !== 'true') {
  import('react-scan')
    .then((ReactScan) => {
      ReactScan.scan({
        enabled: true,
        log: true,
      });
    })
    .catch(() => {
      // React Scan not available in production
    });
}

// Load the appropriate app based on VITE_APP_MODE
const appMode = import.meta.env.VITE_APP_MODE || 'mixer';

const loadApp = async () => {
  let App;

  if (appMode === 'volume-control') {
    App = (await import('./apps/volume-control/App')).default;
  } else {
    App = (await import('./apps/mixer/App')).default;
  }

  const container = document.getElementById('root');
  if (container) {
    const root = createRoot(container);
    root.render(
      <MantineProvider>
        <App />
      </MantineProvider>
    );
  }
};

void loadApp();
