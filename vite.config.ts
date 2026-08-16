import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

import pkg from './package.json' with { type: 'json' };

const host = process.env.TAURI_DEV_HOST;

const isDev = process.env.NODE_ENV !== 'production';

const isTurboRun =
  process.env.npm_lifecycle_event?.startsWith('turbo') ?? process.env.TURBO_HASH !== undefined;

// https://vitejs.dev/config/
export default defineConfig(() => ({
  plugins: [
    react({
      babel: {
        plugins: [['babel-plugin-react-compiler', { target: '19' }]],
      },
    }),
    ...(isDev
      ? [
          {
            name: 'turbo-integration',
            configResolved(config) {
              if (isTurboRun) {
                // Optimize for Turbo pipeline execution
                config.logLevel = 'warn';
                config.clearScreen = false;
              }
            },
          },
        ]
      : []),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri` and turbo cache
      ignored: ['**/src-tauri/**', '**/.turbo/**', '**/node_modules/.cache/**'],
    },
    // Enhanced development server options
    fs: {
      allow: ['..'],
    },
  },

  // 4. optimize for Turbo pipeline
  optimizeDeps: {
    include: ['@mantine/core', '@mantine/hooks', 'lodash'],
    exclude: ['@tauri-apps/api'],
  },

  build: {
    target: ['es2020'],
    // Optimize builds for Turbo caching
    sourcemap: isDev ? ('inline' as const) : false,
    minify: !isDev,
    rollupOptions: {
      output: {
        manualChunks: {
          // UI framework chunk
          ui: ['@mantine/core', '@mantine/hooks'],
          // Utilities chunk
          utils: ['lodash'],
        },
      },
    },
    // Turbo-optimized build settings
    reportCompressedSize: !isTurboRun,
    chunkSizeWarningLimit: 1000,
  },

  // Enhanced resolve configuration
  resolve: {
    alias: {
      '@': '/src',
    },
  },

  // Environment variables for Turbo integration
  define: {
    'process.env.TURBO_RUN': JSON.stringify(isTurboRun.toString()),
    'process.env.BUILD_TIME': JSON.stringify(new Date().toISOString()),
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
}));
