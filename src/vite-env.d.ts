/// <reference types="vite/client" />

type ImportMetaEnv = {
  readonly VITE_APP_TITLE: string;
  readonly REACT_SCAN_ENABLED: string;
  // more env variables...
};

type ImportMeta = {
  readonly env: ImportMetaEnv;
};

/** Injected at build time from package.json. See `define` in vite.config.ts. */
declare const __APP_VERSION__: string;
