# Repository Guidelines

## Project Structure & Module Organization
Sendin Beats pairs a React UI (`src/`) with a Tauri Rust core (`src-tauri/`). Components and hooks live under `src/components` and `src/hooks`, Zustand stores in `src/stores`, and shared types in `src/types`. Backend audio sits in `src-tauri/src/audio/`, persistence in `src-tauri/src/db/`, and migrations in `src-tauri/migrations/`. Auxiliary pieces include Swift capture helpers in `src-swift/` and logs in `logs/`.

## Build & Development Commands
- `pnpm tauri dev --release` – primary desktop loop with live logs.
- `pnpm tauri:dev` – wrapper that rebuilds the Swift helper, clears logs, and launches the same shell.
- `pnpm dev` – Vite-only preview against the most recent backend output.
- `pnpm build` / `pnpm build:tauri` – generate production bundles for web and desktop.
- `turbo rust:check` – run after each change cycle; no automated tests exist yet, so record manual verification in PRs.

## Coding Style & Frontend Practices
TypeScript uses Prettier (2-space, 100-col, single quotes) and the flat ESLint stack; run `pnpm lint` or `pnpm lint:fix` before review. Prefer named exports, type aliases, and descriptive hook names; avoid interfaces, default exports, and any casting—lean on type guards or generics instead. Import modules directly by path rather than through barrel `index` files. Zustand stores expose ID-driven selectors; components fetch data locally. Keep functions <150 lines, modules <800 lines, and comment only when logic is non-obvious. Stick with the established libraries: `@mantine/*` for UI, `zustand` for state, `zod` for validation, `react-hook-form` for inputs, and `@tanstack/react-query` for async caching.

## Backend & Database Conventions
Generate migrations with `pnpm migration <description>` to create `YYYYMMDDHHMMSS_description.sql`. Tables must use `VARCHAR(36)` UUID primary keys, include `created_at` and `updated_at` timestamps, and store enums as `TEXT`. After schema changes, update the matching Rust structs in `src-tauri/src/db/`, keep `sqlx` annotations (e.g., `id as "id: Uuid"`), and run `turbo rust:check`. Introduce new behavior through focused submodules rather than expanding large files.

## Logging Standards
When modifying logging, replace `println!` with structured macros and colored prefixes using `colored::Colorize` (`"RESAMPLER_INIT".blue()`, `"TIMING_DEBUG".on_blue()`). Reuse colors per subsystem so interleaved logs remain scannable.

## Commit & Workflow Discipline
Commits follow emoji-prefixed, imperative subjects (`🔧 Fix mixer sync drift`). Execute `turbo rust:fmt` for any Rust edits, `turbo lint:fix` for touched TypeScript, and `pnpm format` for prose or config changes. Work in small, reviewable slices, ask for feedback early, and include manual validation notes (device routing checks, filtered log snippets) with each PR before requesting review.

## Verifying Changes
After adding or adjusting log statements, capture evidence with the existing filter script: `"KEYWORDS=RESAMPLER_INIT,TIMING_DEBUG" pnpm logs:filter`. Tailor the comma-separated prefixes to the colors you introduced, attach the filtered output to your PR notes, and clear logs afterward with `pnpm logs:clean`.
