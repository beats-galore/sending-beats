import { MantineProvider } from '@mantine/core';
import { createRoot } from 'react-dom/client';

import App from './App';
import './styles.css';
import '@mantine/core/styles.css';

console.log('REACT_SCAN_ENABLED', import.meta.env.REACT_SCAN_ENABLED);
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
      // React Scan not available in production
    });
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
