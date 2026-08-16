import { fileURLToPath } from 'node:url';
import { defineConfig } from 'oxlint';

// Absolute path so the specifier resolves against this file rather than the
// directory oxlint happens to be invoked from.
const localRulesPlugin = fileURLToPath(new URL('./lint-rules/index.mjs', import.meta.url));

/**
 * Oxlint config for the Sweet Beats Studio frontend.
 *
 * This replaces the old `eslint.config.mjs`. No ESLint package remains in the
 * dependency tree: unused imports/vars are covered by oxlint's native
 * `typescript/no-unused-vars`, and naming-convention is a local pure-AST rule
 * in lint-rules/ loaded through the jsPlugin bridge.
 *
 * Formatting stays with Prettier — oxlint has no formatting rules, so the old
 * `eslint-config-prettier` layer is no longer needed.
 */
export default defineConfig({
  plugins: ['typescript', 'import', 'react'],
  jsPlugins: [{ name: 'local', specifier: localRulesPlugin }],
  categories: {
    correctness: 'off',
  },
  options: {
    typeAware: true,
  },
  env: {
    builtin: true,
    es2024: true,
    browser: true,
    node: true,
  },
  ignorePatterns: [
    '**/node_modules/',
    '**/dist/',
    '**/build/',
    '**/coverage/',
    '**/out/',
    '**/.turbo/',
    'src-tauri/',
    'logs/',
    'screenshots/',
    'templates/',
    '**/*.gen.ts',
    '**/*.gen.js',
    '**/*.min.js',
    'pnpm-lock.yaml',
    // Lint/build config files are not in a tsconfig project, so type-aware
    // linting cannot resolve them.
    'oxlint.config.mts',
    'lint-rules/',
  ],
  rules: {
    // --- unused code ---
    // Covers unused imports too. `--fix` leaves them alone because removing an
    // import can change behaviour (side-effect modules); `lint:fix` passes
    // `--fix-dangerously` so they are stripped automatically.
    'typescript/no-unused-vars': [
      'error',
      {
        vars: 'all',
        varsIgnorePattern: '^_',
        args: 'after-used',
        argsIgnorePattern: '^_',
        caughtErrorsIgnorePattern: '^_',
      },
    ],

    // --- eslint core (the `correctness` category is off, so enable explicitly) ---
    'constructor-super': 'error',
    'for-direction': 'error',
    'getter-return': 'error',
    'no-async-promise-executor': 'error',
    'no-case-declarations': 'error',
    'no-class-assign': 'error',
    'no-compare-neg-zero': 'error',
    'no-cond-assign': 'error',
    'no-const-assign': 'error',
    'no-constant-binary-expression': 'warn',
    'no-constant-condition': 'error',
    'no-control-regex': 'error',
    'no-debugger': 'error',
    'no-delete-var': 'error',
    'no-dupe-class-members': 'error',
    'no-dupe-else-if': 'error',
    'no-dupe-keys': 'error',
    'no-duplicate-case': 'error',
    'no-empty': 'error',
    'no-empty-character-class': 'error',
    'no-empty-pattern': 'error',
    'no-empty-static-block': 'error',
    'no-ex-assign': 'error',
    'no-extra-boolean-cast': 'error',
    'no-fallthrough': 'error',
    'no-func-assign': 'error',
    'no-global-assign': 'error',
    'no-import-assign': 'error',
    'no-invalid-regexp': 'error',
    'no-irregular-whitespace': 'error',
    'no-loss-of-precision': 'error',
    'no-misleading-character-class': 'error',
    'no-new-native-nonconstructor': 'error',
    'no-nonoctal-decimal-escape': 'error',
    'no-obj-calls': 'error',
    'no-prototype-builtins': 'error',
    'no-redeclare': 'error',
    'no-regex-spaces': 'error',
    'no-self-assign': 'error',
    'no-setter-return': 'error',
    'no-shadow-restricted-names': 'error',
    'no-sparse-arrays': 'error',
    'no-this-before-super': 'error',
    'no-unexpected-multiline': 'error',
    'no-unreachable': 'error',
    'no-unsafe-finally': 'error',
    'no-unsafe-negation': 'error',
    'no-unsafe-optional-chaining': 'warn',
    'no-unused-labels': 'error',
    'no-unused-private-class-members': 'error',
    'no-useless-backreference': 'error',
    'no-useless-catch': 'error',
    'no-useless-escape': 'error',
    'no-with': 'error',
    'require-yield': 'error',
    'use-isnan': 'error',
    'valid-typeof': 'error',
    'no-array-constructor': 'error',
    'no-var': 'error',
    'prefer-const': 'error',
    'prefer-rest-params': 'warn',
    'prefer-spread': 'error',

    // --- house style (was in eslint.config.mjs) ---
    eqeqeq: ['error', 'always'],
    curly: ['error', 'all'],
    'no-param-reassign': ['error', { props: false }],

    // --- typescript ---
    'typescript/ban-ts-comment': 'off',
    'typescript/no-duplicate-enum-values': 'error',
    'typescript/no-empty-object-type': 'error',
    'typescript/no-explicit-any': 'warn',
    'typescript/no-extra-non-null-assertion': 'error',
    'typescript/no-misused-new': 'error',
    'typescript/no-namespace': 'error',
    'typescript/no-non-null-asserted-optional-chain': 'error',
    'typescript/no-require-imports': 'error',
    'typescript/no-this-alias': 'error',
    'typescript/no-unnecessary-type-constraint': 'error',
    'typescript/no-unsafe-declaration-merging': 'error',
    'typescript/no-unsafe-function-type': 'error',
    'typescript/no-wrapper-object-types': 'error',
    'typescript/prefer-as-const': 'error',
    'typescript/prefer-namespace-keyword': 'error',
    'typescript/triple-slash-reference': 'error',
    'typescript/no-unnecessary-template-expression': 'error',
    'typescript/consistent-type-imports': 'warn',
    'typescript/no-shadow': 'error',

    // type-aware rules (need `options.typeAware` + `--type-aware`)
    'typescript/no-unnecessary-condition': 'warn',
    'typescript/no-misused-promises': 'warn',
    'typescript/return-await': 'error',

    // typeLike → PascalCase, formerly `@typescript-eslint/naming-convention`.
    // See lint-rules/index.mjs for why this is a local rule.
    'local/naming-convention': 'error',

    // --- import ---
    'import/no-default-export': 'warn',
    'import/no-commonjs': 'error',

    // --- react (oxlint's react plugin includes the react-hooks rules) ---
    'react/rules-of-hooks': 'error',
    'react/exhaustive-deps': 'error',
    'react/jsx-key': 'error',
    'react/jsx-no-duplicate-props': 'error',
    'react/jsx-no-constructed-context-values': 'warn',
    'react/no-unstable-nested-components': 'warn',
    'react/jsx-filename-extension': ['error', { extensions: ['.jsx', '.tsx'] }],
    // Not ported by oxlint, so dropped from the eslint config: `react/jsx-no-bind`,
    // `react/no-unused-class-component-methods`, `import/order`.
  },
  overrides: [
    {
      // Default-export is the convention for build/config files.
      files: ['vite.config.ts', '*.config.ts', '*.config.mts'],
      rules: {
        'import/no-default-export': 'off',
      },
    },
    {
      // Plain JS / config files may use CommonJS.
      files: ['**/*.js', '**/*.cjs', '**/*.mjs'],
      rules: {
        'import/no-commonjs': 'off',
        'typescript/no-require-imports': 'off',
      },
    },
  ],
});
