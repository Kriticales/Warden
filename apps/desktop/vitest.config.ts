import { defineConfig, mergeConfig } from 'vitest/config';

import viteConfig from './vite.config';

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test/setup.ts'],
      include: ['src/**/*.test.{ts,tsx}', '*.test.ts'],
      restoreMocks: true,
      coverage: {
        provider: 'v8',
        include: ['src/**/*.{ts,tsx}'],
        exclude: [
          'src/**/*.test.{ts,tsx}',
          'src/lib/ipc/bindings.ts',
          'src/routeTree.gen.ts',
          'src/test/**',
        ],
        // Mínimos da QUALITY §4.2 (linhas), aplicados por `cargo xtask coverage`.
        thresholds: {
          'src/lib/**': { lines: 85 },
          'src/features/*/lib/**': { lines: 85 },
          'src/features/*/hooks/**': { lines: 70 },
        },
      },
    },
  }),
);
