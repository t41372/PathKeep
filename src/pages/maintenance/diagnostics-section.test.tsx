/**
 * @file diagnostics-section.test.tsx
 * @description Focused coverage for Maintenance-owned diagnostics and support path review rows.
 * @module pages/maintenance
 *
 * ## Responsibilities
 * - Verify support paths, build metadata, MCP posture, and crash-report affordances render from shell snapshot truth.
 * - Protect copy/open-path callbacks on local support rows.
 *
 * ## Not responsible for
 * - Re-testing Settings route-state loading or repair workflows.
 * - Re-testing shared review-row internals beyond the diagnostics panel integration.
 *
 * ## Dependencies
 * - Uses the preview backend harness for a production-shaped app snapshot.
 * - Uses the shipped i18n provider for visible maintenance/settings copy.
 *
 * ## Performance notes
 * - Runs without booting the full Maintenance route, keeping diagnostics coverage bounded to this panel.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { I18nProvider } from '../../lib/i18n'
import type { AppBuildInfo, AppSnapshot } from '../../lib/types'
import { DiagnosticsSection } from './diagnostics-section'

const buildInfo: AppBuildInfo = {
  productName: 'PathKeep',
  version: '0.1.0',
  gitCommitShort: 'abc1234',
  gitCommitFull: 'abc1234567890',
  gitDirty: true,
}

describe('DiagnosticsSection', () => {
  test('renders support paths and clear crash status', async () => {
    const user = userEvent.setup()
    const snapshot = await createSnapshot()
    const onCopyPath = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn()

    renderDiagnostics(snapshot, { onCopyPath, onOpenPath })

    expect(screen.getByText('Support diagnostics')).toBeVisible()
    expect(screen.getByText(snapshot.directories.appRoot)).toBeVisible()
    expect(
      screen.getByText(snapshot.directories.archiveDatabasePath),
    ).toBeVisible()
    expect(screen.getByText('No recent crash report')).toBeVisible()
    expect(screen.getByText('0.1.0')).toBeVisible()
    expect(screen.getByText('abc1234+')).toBeVisible()
    expect(screen.getByText('Off')).toBeVisible()

    const copyButtons = screen.getAllByRole('button', { name: 'Copy' })
    for (const button of copyButtons) {
      await user.click(button)
    }
    expect(onCopyPath).toHaveBeenCalledWith(
      'maintenance:app-root',
      snapshot.directories.appRoot,
    )
    expect(onCopyPath).toHaveBeenCalledWith(
      'maintenance:archive-database',
      snapshot.directories.archiveDatabasePath,
    )
    expect(onCopyPath).toHaveBeenCalledWith(
      'maintenance:audit-repo',
      snapshot.directories.auditRepoPath,
    )
    expect(onCopyPath).toHaveBeenCalledWith(
      'maintenance:logs-dir',
      snapshot.directories.logsDir,
    )
    expect(onCopyPath).toHaveBeenCalledWith(
      'maintenance:crash-reports',
      snapshot.directories.crashReportsDir,
    )
  })

  test('surfaces latest crash report and opens the concrete crash artifact', async () => {
    const user = userEvent.setup()
    const snapshot = await createSnapshot({
      latestCrashReport: {
        source: 'rust-panic',
        recordedAt: '2026-04-25T12:00:00.000Z',
        fatal: true,
        message: 'panic while importing browser history',
        location: 'src-tauri/src/lib.rs:42',
        path: '/Users/test/Library/Application Support/PathKeep/crash.json',
      },
    })
    const onOpenPath = vi.fn()

    renderDiagnostics(snapshot, { onOpenPath })

    expect(screen.getByText('Recent crash report detected')).toBeVisible()
    expect(
      screen.getByText(/panic while importing browser history/),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Open crash report' }))
    expect(onOpenPath).toHaveBeenCalledWith(
      '/Users/test/Library/Application Support/PathKeep/crash.json',
    )
  })

  test('falls back to crash-report directory and unavailable build labels', async () => {
    const user = userEvent.setup()
    const snapshot = await createSnapshot({
      latestCrashReport: {
        source: 'frontend-error',
        recordedAt: 'not-a-date',
        fatal: false,
        message: 'render failed before route recovery',
        location: null,
        path: null,
      } as unknown as AppSnapshot['runtimeDiagnostics']['latestCrashReport'],
    })
    snapshot.config.ai.mcpEnabled = true
    const onOpenPath = vi.fn()

    renderDiagnostics(snapshot, { buildInfo: null, onOpenPath })

    expect(screen.getByText('On')).toBeVisible()
    expect(screen.getAllByText('Not available').length).toBeGreaterThanOrEqual(
      2,
    )
    expect(
      screen.getByText(/Frontend crash recorded at not-a-date/),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Open crash report' }))
    expect(onOpenPath).toHaveBeenCalledWith(
      snapshot.directories.crashReportsDir,
    )
  })
})

async function createSnapshot({
  latestCrashReport = null,
}: Pick<AppSnapshot['runtimeDiagnostics'], 'latestCrashReport'> = {}) {
  backendTestHarness.reset()
  const snapshot = await backend.getAppSnapshot()

  return {
    ...snapshot,
    config: {
      ...snapshot.config,
      ai: {
        ...snapshot.config.ai,
        mcpEnabled: false,
      },
    },
    runtimeDiagnostics: {
      ...snapshot.runtimeDiagnostics,
      latestCrashReport,
    },
  }
}

function renderDiagnostics(
  snapshot: AppSnapshot,
  {
    buildInfo: buildInfoOverride = buildInfo,
    onCopyPath = vi.fn().mockResolvedValue(undefined),
    onOpenPath = vi.fn(),
  }: {
    buildInfo?: AppBuildInfo | null
    onCopyPath?: (key: string, value: string) => Promise<void>
    onOpenPath?: (path: string) => void
  } = {},
) {
  return render(
    <I18nProvider>
      <DiagnosticsSection
        buildInfo={buildInfoOverride}
        copyFeedback={null}
        onCopyPath={onCopyPath}
        onOpenPath={onOpenPath}
        snapshot={snapshot}
      />
    </I18nProvider>,
  )
}
