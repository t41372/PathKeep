import path from 'node:path'
import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

const rootDir = path.dirname(fileURLToPath(import.meta.url))

const qualitySurface = [
  'src/main.tsx',
  'src/app/shell-data.tsx',
  'src/lib/backend.ts',
  'src/lib/format.ts',
  'src/lib/intelligence.ts',
  'src/lib/ipc/bridge.ts',
  'src/lib/platform-guidance.ts',
  'src/lib/stronghold.ts',
  'src/lib/trust-review.ts',
  'src/lib/i18n/context.ts',
  'src/lib/i18n/hooks.ts',
  'src/lib/i18n/provider.tsx',
]

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    testTimeout: 15_000,
    hookTimeout: 15_000,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    exclude: ['tests/e2e/**'],
    coverage: {
      // @ts-expect-error Vitest runtime supports `all`; the bundled type defs lag.
      all: true,
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      reportsDirectory: './coverage/js-quality',
      include: qualitySurface,
      exclude: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
      thresholds: {
        lines: 100,
        functions: 100,
        branches: 100,
        statements: 100,
      },
    },
  },
  resolve: {
    alias: {
      '@': path.join(rootDir, 'src'),
    },
  },
})
