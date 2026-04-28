import path from 'node:path'
import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

const rootDir = path.dirname(fileURLToPath(import.meta.url))

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
      reportsDirectory: './coverage/js',
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.d.ts',
        'src/**/*.test.ts',
        'src/**/*.test.tsx',
        'src/**/*fixtures.ts',
        'src/**/*fixtures.tsx',
        'src/assets/**',
        'src/test/**',
        'src/**/test-helpers.ts',
        'src/**/test-helpers.tsx',
        'src/lib/types/**',
        'src/components/intelligence/workbench/index.ts',
        'src/components/intelligence/workbench/review-surface.tsx',
        'src/components/review/index.ts',
        'src/lib/core-intelligence/api.ts',
        'src/lib/core-intelligence/index.ts',
        'src/lib/core-intelligence/api/index.ts',
        'src/lib/core-intelligence/api/overview.ts',
        'src/lib/core-intelligence/types*.ts',
        'src/lib/i18n.ts',
        'src/lib/i18n/catalog.ts',
        'src/lib/intelligence.ts',
        'src/pages/explorer/types.ts',
        'src/pages/intelligence/promoted-entity-routes.tsx',
        'src/pages/intelligence/sections/secondary-sections.tsx',
      ],
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
