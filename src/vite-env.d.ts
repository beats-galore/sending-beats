/// <reference types="vite/client" />

// Declaration merging is the only way to extend Vite's own ImportMetaEnv, and
// merging requires an interface, so this is the one place the codebase's
// preference for type literals does not apply.
interface ImportMetaEnv {
  /** Set to 'true' to load React Scan in a dev build. Off unless opted in. */
  readonly VITE_REACT_SCAN?: string;
}

/** Injected at build time from package.json. See `define` in vite.config.ts. */
declare const __APP_VERSION__: string;
