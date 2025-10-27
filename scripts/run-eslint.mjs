#!/usr/bin/env node
/* eslint-disable */
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// Collect CLI args passed after the script name
const rawArgs = process.argv.slice(2);

// If no positional args (non-flags) are provided, default to current project
const hasPositional = rawArgs.some((arg) => !arg.startsWith('-'));
const eslintArgs = hasPositional ? rawArgs : ['.', ...rawArgs];

// Resolve local eslint binary to ensure we use the project's version
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const eslintBin = path.resolve(__dirname, '../node_modules/.bin/eslint');

const result = spawnSync(eslintBin, eslintArgs, {
  stdio: 'inherit',
  env: process.env,
});

process.exit(result.status ?? 1);
