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
        'src/components/ui/button.tsx',
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
        'src/pages/explorer/types.ts',
        'src/pages/intelligence/promoted-entity-routes.tsx',
        'src/pages/intelligence/sections/secondary-sections.tsx',
      ],
      // 99% — calibrated to the v0.3 paper-redesign achievable state after
      // the orphan sweep. The residual ~1% lives in the legacy explorer
      // layout=legacy branch (Phase 4 retires it), dashboard/shell helper
      // catch fallbacks (defensive `new Date(...)` paths the Date
      // constructor doesn't actually throw on), and 1-line gaps inside
      // explorer-paper components. Each file at <100% is enumerated in
      // BACKLOG.md ("WORK-V03-COVERAGE-RESIDUAL"); the next sweep raises
      // the floor as Phase 4 + the explorer-paper hardening lands. Do not
      // lower further without an additional backlog item.
      thresholds: {
        lines: 99,
        functions: 98,
        branches: 98,
        statements: 98,
      },
    },
  },
  resolve: {
    alias: {
      '@': path.join(rootDir, 'src'),
    },
  },
})
