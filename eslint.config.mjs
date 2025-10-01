import { FlatCompat } from '@eslint/eslintrc';
import { fixupConfigRules } from '@eslint/compat';
import eslint from '@eslint/js';
import tseslintPlugin from '@typescript-eslint/eslint-plugin';
import importPlugin from 'eslint-plugin-import';
import reactPlugin from 'eslint-plugin-react';
import reactHooksPlugin from 'eslint-plugin-react-hooks';
import unusedImports from 'eslint-plugin-unused-imports';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import tseslint from 'typescript-eslint';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const compat = new FlatCompat({
  baseDirectory: __dirname,
});

export const extendableBaseConfig = [
  eslint.configs.recommended,
  {
    ignores: ['**/node_modules', '**/*.json', '**/dist/**', 'eslint.config.mjs'],
  },
  ...compat.extends('prettier'),
  // Apply non-type-aware base rules globally; scope type-aware rules only to TS files below
  ...tseslint.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],

    plugins: {
      '@typescript-eslint': tseslintPlugin,
      'unused-imports': unusedImports,

      import: importPlugin,
      'react-hooks': reactHooksPlugin,
    },
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        project: './tsconfig.eslint.json',
        tsconfigRootDir: __dirname,
        ecmaVersion: 'latest',
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
    settings: {
      'import/resolver': {
        typescript: {
          project: './src',
        },
        node: {
          moduleDirectory: ['node_modules', './'],
          extensions: ['.d.ts', '.ts', '.tsx', '.js', '.jsx', '.json'],
          paths: ['src'],
        },
      },
      'import/parsers': {
        '@typescript-eslint/parser': ['.ts', '.tsx'],
      },
    },
    rules: {
      'eslint-comments/no-unused-disable': 'off',
      '@typescript-eslint/consistent-type-imports': 'warn',
      camelcase: 'off',
      'no-unsafe-optional-chaining': 'warn',
      'no-unused-vars': 'off',
      'unused-imports/no-unused-vars': 'off',
      'unused-imports/no-unused-imports': 'error',
      curly: ['error', 'all'],
      'import/extensions': 'off',
      eqeqeq: ['error', 'always'],
      'max-len': 'off',
      'no-console': 'off',
      'no-restricted-syntax': 'off',
      'no-await-in-loop': 'off',
      'no-continue': 'off',
      'no-void': 'off',
      'prefer-destructuring': 'off',
      'class-methods-use-this': 'off',
      'max-classes-per-file': 'off',
      '@typescript-eslint/no-var-requires': ['error'],
      '@typescript-eslint/no-empty-function': ['off'],
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          varsIgnorePattern: '^_',
          argsIgnorePattern: '^_',
        },
      ],
      '@typescript-eslint/no-unnecessary-condition': ['warn'],
      '@typescript-eslint/no-require-imports': 'error',
      '@typescript-eslint/return-await': 'error',
      '@typescript-eslint/no-shadow': ['error'],
      '@typescript-eslint/no-explicit-any': 'warn',
      'no-use-before-define': 'off',
      'no-underscore-dangle': 'off',
      '@typescript-eslint/no-use-before-define': ['off'],
      'no-param-reassign': [
        'error',
        {
          props: false,
        },
      ],
      'no-plusplus': 'off',
      'prefer-rest-params': 'warn',
      'import/prefer-default-export': 'off',
      'import/no-cycle': 'off',
      'import/no-default-export': 'warn',
      'import/no-commonjs': 'error',
      '@typescript-eslint/no-misused-promises': 'warn',
      '@typescript-eslint/naming-convention': [
        'error',
        {
          selector: ['typeLike'],
          format: ['PascalCase'],
        },
      ],
      'import/order': [
        'error',
        {
          groups: [['builtin', 'external'], 'internal', 'parent', 'sibling', 'index', 'object'],
          alphabetize: {
            order: 'asc',
            caseInsensitive: true,
          },
        },
      ],
      'import/no-restricted-paths': ['error'],
      'no-restricted-imports': ['error'],
    },
  },
  {
    files: ['**/*.js'],
    rules: {
      'import/no-commonjs': 'off',
    },
  },
  {
    files: ['**/*.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-require-imports': [
        'error',
        {
          allow: ['.*\\.(png|jpg|gif|svg)$'],
        },
      ],
      '@typescript-eslint/no-var-requires': [
        'error',
        {
          allow: ['.*\\.(png|jpg|gif|svg)$'],
        },
      ],
    },
  },
];

export default tseslint.config(extendableBaseConfig);
