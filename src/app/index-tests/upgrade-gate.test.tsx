/**
 * @file upgrade-gate.test.tsx
 * @description Coverage for the AppBody upgrade gate: when `archiveUpgrade` is non-null,
 * AppBody renders ArchiveUpgradeScreen instead of the router.
 * @module app/index-tests
 *
 * ## What this suite covers
 * - AppBody renders ArchiveUpgradeScreen when `useShellData().archiveUpgrade` is non-null.
 * - AppBody renders the RouterProvider when archiveUpgrade is null.
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
import type { AppConfig, ArchiveUpgradeAssessment } from '../../lib/types'
import { AppBody } from '../index'

// ──────────────────────────────────────────────────────────────────────────────
// Mock backend + progress subscribe so the mounted ArchiveUpgradeScreen never
// fires real IPC. `initialize_archive` stays pending so the screen holds its
// working state for the duration of the assertion.
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      initializeArchive: vi.fn().mockReturnValue(new Promise(() => {})),
    },
  }
})

vi.mock('../../lib/ipc/archive-upgrade-progress', () => ({
  subscribeToArchiveUpgradeProgress: vi.fn().mockResolvedValue(() => {}),
}))

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

function makeUpgrade(): {
  assessment: ArchiveUpgradeAssessment
  config: AppConfig
} {
  return {
    assessment: {
      pending: true,
      currentSchemaVersion: 14,
      targetSchemaVersion: 16,
      phases: [
        {
          phase: 'registrableDomainBackfill',
          phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
          pending: true,
          streamed: true,
          estimatedTotal: 12000,
        },
      ],
    },
    config: { initialized: true } as AppConfig,
  }
}

function makeShellValue(
  archiveUpgrade: ShellDataContextValue['archiveUpgrade'],
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
    recovery: null,
    archiveUpgrade,
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

function renderAppBody(
  archiveUpgrade: ShellDataContextValue['archiveUpgrade'],
) {
  const router = createMemoryRouter([
    { path: '*', element: <div data-testid="router-content">Router</div> },
  ])
  return render(
    <I18nProvider>
      <ShellDataContext.Provider value={makeShellValue(archiveUpgrade)}>
        <AppBody router={router} />
      </ShellDataContext.Provider>
    </I18nProvider>,
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('AppBody upgrade gate', () => {
  test('renders ArchiveUpgradeScreen when archiveUpgrade is non-null', () => {
    renderAppBody(makeUpgrade())
    expect(screen.getByTestId('archive-upgrade-screen')).toBeInTheDocument()
    expect(screen.queryByTestId('router-content')).not.toBeInTheDocument()
  })

  test('renders RouterProvider content when archiveUpgrade is null', async () => {
    renderAppBody(null)
    expect(await screen.findByTestId('router-content')).toBeInTheDocument()
    expect(
      screen.queryByTestId('archive-upgrade-screen'),
    ).not.toBeInTheDocument()
  })
})
