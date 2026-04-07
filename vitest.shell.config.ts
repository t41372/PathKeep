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
    include: [
      'src/main.test.tsx',
      'src/app/**/*.test.tsx',
      'src/components/sidebar/**/*.test.tsx',
      'src/components/topbar/**/*.test.tsx',
      'src/components/primitives/**/*.test.tsx',
      'src/lib/tokens.test.ts',
      'src/lib/ipc/**/*.test.ts',
    ],
    exclude: ['tests/e2e/**'],
    coverage: {
      // @ts-expect-error Vitest runtime supports `all`; the bundled type defs lag.
      all: true,
      provider: 'v8',
      reporter: ['text'],
      reportsDirectory: './coverage/js-shell',
      include: [
        'src/main.tsx',
        'src/app/**/*.tsx',
        'src/components/sidebar/**/*.tsx',
        'src/components/topbar/**/*.tsx',
        'src/components/primitives/**/*.tsx',
        'src/lib/tokens.ts',
        'src/lib/ipc/bridge.ts',
        'src/pages/dashboard/**/*.tsx',
        'src/pages/explorer/**/*.tsx',
        'src/pages/insights/**/*.tsx',
        'src/pages/assistant/**/*.tsx',
        'src/pages/import/**/*.tsx',
        'src/pages/audit/**/*.tsx',
        'src/pages/schedule/**/*.tsx',
        'src/pages/security/**/*.tsx',
        'src/pages/settings/**/*.tsx',
        'src/pages/onboarding/**/*.tsx',
      ],
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
