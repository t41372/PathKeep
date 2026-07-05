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
        'src/components/assistant-chat/index.ts',
        'src/components/cards/index.ts',
        'src/components/explorer-paper/index.ts',
        'src/components/intelligence/workbench/index.ts',
        'src/components/intelligence/workbench/review-surface.tsx',
        'src/components/review/index.ts',
        'src/components/shell/index.ts',
        // shadcn-derived primitives that survive only because the paper shell
        // search-palette and status-bar still wrap them. We exercise the
        // exact code paths in use through pk-search-palette / pk-status-bar
        // tests; the rest is upstream variant logic we don't ship. Treat as
        // third-party shim surface, same precedent as
        // intelligence/workbench/review-surface.tsx.
        //
        // button.tsx is EXCLUDED from this list on purpose: it now carries
        // real paper-token variant/size logic plus the `loading` affordance,
        // so it has its own direct button.test.tsx instead of relying on
        // incidental coverage from a wrapper.
        'src/components/ui/command.tsx',
        'src/components/ui/dialog.tsx',
        'src/components/ui/popover.tsx',
        'src/lib/core-intelligence/api.ts',
        'src/lib/core-intelligence/index.ts',
        'src/lib/core-intelligence/api/index.ts',
        'src/lib/core-intelligence/api/overview.ts',
        'src/lib/core-intelligence/types*.ts',
        'src/lib/i18n.ts',
        'src/lib/i18n/catalog.ts',
        'src/lib/intelligence.ts',
        // Pure TypeScript type-only file (no runtime code — only interfaces and type aliases).
        'src/pages/jobs/activity-types.ts',
        'src/pages/explorer/types.ts',
        'src/pages/intelligence/promoted-entity-routes.tsx',
        'src/pages/intelligence/sections/secondary-sections.tsx',
      ],
      // Runtime JS/TS coverage is a hard gate. TEST_PLAN.md Module 10G closes
      // the residual sweep and confirms lcov has no uncovered lines, branches,
      // or functions. Do not lower thresholds without a matching bug/drift
      // entry and a documented rollback reason.
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
