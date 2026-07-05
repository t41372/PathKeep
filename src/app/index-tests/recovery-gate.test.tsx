/**
 * @file recovery-gate.test.tsx
 * @description Coverage for the AppBody recovery gate: when `recovery` is non-null,
 * AppBody renders ArchiveRecoveryScreen instead of the router.
 * @module app/index-tests
 *
 * ## What this suite covers
 * - AppBody renders ArchiveRecoveryScreen when `useShellData().recovery` is non-null.
 * - AppBody renders the RouterProvider when recovery is null.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { createMemoryRouter } from 'react-router-dom'
import { I18nProvider } from '../../lib/i18n'
import type * as BackendClient from '../../lib/backend-client'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../shell-data-context'
import type { ArchiveRecoveryReport } from '../../lib/types'
import { AppBody } from '../index'

// ──────────────────────────────────────────────────────────────────────────────
// Mock backend so IPC calls from ArchiveRecoveryScreen don't fire for real.
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      previewSnapshotRestore: vi.fn().mockResolvedValue({
        snapshotPath: '/snap.sqlite',
        snapshotKind: 'backup',
        createdAt: null,
        executeSupported: true,
        estimatedVisits: 0,
        estimatedUrls: 0,
        estimatedDownloads: 0,
        warnings: [],
      }),
      revealLogs: vi.fn().mockResolvedValue(undefined),
    },
  }
})

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

function makeShellValue(
  recovery: ArchiveRecoveryReport | null,
): ShellDataContextValue {
  return {
    buildInfo: null,
    appLockStatus: null,
    snapshot: null,
    dashboard: null,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    rawError: null,
    notice: null,
    refreshKey: 0,
    errorKind: null,
    recovery,
    archiveUpgrade: null,
    finishArchiveUpgrade: vi.fn().mockResolvedValue(undefined),
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue({}),
    saveConfig: vi.fn().mockResolvedValue({}),
    initializeArchive: vi.fn(),
    runBackup: vi.fn().mockResolvedValue({}),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    lockAppSession: vi.fn().mockResolvedValue({}),
    unlockAppSession: vi.fn().mockResolvedValue({}),
    startLocalSemanticSetup: vi.fn().mockResolvedValue(undefined),
    clearNotice: vi.fn(),
    clearError: vi.fn(),
    runFullArchiveRestore: vi.fn().mockResolvedValue({}),
  } as ShellDataContextValue
}

function makeRecovery(): ArchiveRecoveryReport {
  return {
    kind: 'atRestDriftUnresolved',
    configMode: 'Plaintext',
    availableSnapshots: ['/snap.sqlite'],
    recoverySnapshots: [
      {
        id: 'snap-1',
        path: '/snap.sqlite',
        createdAt: '2026-06-01T10:00:00Z',
        sizeBytes: 1024,
        verifiedOpenable: true,
        encrypted: false,
        sourceOp: 'backup',
        label: 'Backup',
      },
    ],
    detail: 'archive drift',
  }
}

function renderAppBody(recovery: ArchiveRecoveryReport | null) {
  const router = createMemoryRouter([
    { path: '*', element: <div data-testid="router-content">Router</div> },
  ])
  return render(
    <I18nProvider>
      <ShellDataContext.Provider value={makeShellValue(recovery)}>
        <AppBody router={router} />
      </ShellDataContext.Provider>
    </I18nProvider>,
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('AppBody recovery gate', () => {
  test('renders ArchiveRecoveryScreen when recovery is non-null', () => {
    renderAppBody(makeRecovery())
    expect(screen.getByTestId('archive-recovery-screen')).toBeInTheDocument()
    expect(screen.queryByTestId('router-content')).not.toBeInTheDocument()
  })

  test('renders RouterProvider content when recovery is null', async () => {
    renderAppBody(null)
    expect(await screen.findByTestId('router-content')).toBeInTheDocument()
    expect(
      screen.queryByTestId('archive-recovery-screen'),
    ).not.toBeInTheDocument()
  })
})
