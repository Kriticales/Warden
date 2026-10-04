// ESLint do app (QUALITY §2.2): typescript-eslint strictTypeChecked + stylisticTypeChecked,
// React, react-hooks, jsx-a11y (strict) e i18next (texto solto em JSX é erro).
import js from '@eslint/js';
import { defineConfig, globalIgnores } from 'eslint/config';
import i18next from 'eslint-plugin-i18next';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default defineConfig([
  // Gerados: nunca editados à mão nem verificados.
  globalIgnores([
    'dist/',
    'coverage/',
    'src-tauri/',
    'src/lib/ipc/bindings.ts',
    'src/routeTree.gen.ts',
  ]),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.strictTypeChecked,
      tseslint.configs.stylisticTypeChecked,
      react.configs.flat.recommended,
      react.configs.flat['jsx-runtime'],
      reactHooks.configs.flat.recommended,
      jsxA11y.flatConfigs.strict,
    ],
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    settings: { react: { version: 'detect' } },
    rules: {
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',
      '@typescript-eslint/switch-exhaustiveness-check': 'error',
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-misused-promises': 'error',
      '@typescript-eslint/consistent-type-imports': 'error',
      // Usar o logger que encaminha ao tauri-plugin-log (F0-05).
      'no-console': 'error',
      'react/no-danger': 'error',
      // `invoke` só dentro de src/lib/ipc/ (bindings gerados e wrappers).
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: '@tauri-apps/api/core',
              message: 'Use os comandos gerados em src/lib/ipc/bindings.ts.',
            },
          ],
        },
      ],
    },
  },
  {
    // Texto visível só pelo catálogo pt-BR (QUALITY §8.1; ADR-0016).
    files: ['src/**/*.tsx'],
    plugins: { i18next },
    rules: {
      'i18next/no-literal-string': ['error', { mode: 'jsx-only' }],
    },
  },
  {
    files: ['src/lib/ipc/**'],
    rules: { 'no-restricted-imports': 'off' },
  },
  {
    // Único lugar com HTML cru, já higienizado (F0-06).
    files: ['src/components/common/SafeHtml.tsx'],
    rules: { 'react/no-danger': 'off' },
  },
  {
    files: ['*.{js,ts}'],
    languageOptions: { globals: globals.node },
  },
  {
    files: ['**/*.js'],
    extends: [js.configs.recommended],
    languageOptions: { globals: globals.node },
  },
]);
