/**
 * Smoke tests for the v0.3 paper-redesign Dashboard route.
 *
 * Covers:
 * - Loading state path through the existing route-fallback.
 * - Ready state renders hero band, On This Day card, This Week card, year
 *   heatmap, active threads, archive card, and the local-first footer.
 *
 * Out of scope:
 * - Deep wiring of On This Day backend (covered by core-intelligence tests).
 * - Heatmap density correctness (covered by year-heatmap test).
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '@/app/shell-data-context'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import { DashboardPage } from './index'
import type { AppSnapshot, DashboardSnapshot } from '@/lib/types'

vi.mock('@/lib/core-intelligence/api', () => ({
  getOnThisDay: vi.fn().mockResolvedValue({ data: [] }),
}))

describe('DashboardPage (paper redesign)', () => {
  test('renders the loading fallback when dashboard data is not yet ready', () => {
    renderDashboard({ dashboard: null, dashboardLoading: true })
    // The ready layout's bespoke cards are absent during the fallback.
    expect(screen.queryByTestId('dashboard-on-this-day')).toBeNull()
    expect(screen.queryByTestId('dashboard-archive-card')).toBeNull()
  })

  test('renders the paper layout when snapshot + dashboard are ready', () => {
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    expect(screen.getByTestId('dashboard-page')).toBeInTheDocument()
    expect(screen.getByTestId('dashboard-on-this-day')).toBeInTheDocument()
    expect(screen.getByTestId('dashboard-this-week')).toBeInTheDocument()
    expect(screen.getByTestId('dashboard-active-threads')).toBeInTheDocument()
    expect(screen.getByTestId('dashboard-archive-card')).toBeInTheDocument()
  })
})

function renderDashboard(overrides: Partial<ShellDataContextValue>) {
  return render(
    <I18nProvider>
      <ProfileScopeProvider>
        <MemoryRouter>
          <ShellDataContext.Provider value={makeShellValue(overrides)}>
            <DashboardPage />
          </ShellDataContext.Provider>
        </MemoryRouter>
      </ProfileScopeProvider>
    </I18nProvider>,
  )
}

function makeShellValue(
  overrides: Partial<ShellDataContextValue>,
): ShellDataContextValue {
  return {
    buildInfo: null,
    appLockStatus: null,
    snapshot: null,
    dashboard: null,
    dashboardLoading: false,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    notice: null,
    refreshKey: 0,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn(),
    saveConfig: vi.fn(),
    initializeArchive: vi.fn(),
    runBackup: vi.fn().mockResolvedValue({}),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    lockAppSession: vi.fn().mockResolvedValue({}),
    unlockAppSession: vi.fn(),
    clearNotice: vi.fn(),
    ...overrides,
  } as ShellDataContextValue
}

function makeSnapshot(): AppSnapshot {
  return {
    directories: {} as AppSnapshot['directories'],
    runtimeDiagnostics: {} as AppSnapshot['runtimeDiagnostics'],
    config: {
      initialized: true,
      archiveMode: 'Plaintext',
      selectedProfileIds: [],
      rememberDatabaseKeyInKeyring: false,
      ai: {},
    } as unknown as AppSnapshot['config'],
    archiveStatus: {
      initialized: true,
      encrypted: false,
      unlocked: true,
      databasePath: '~/PathKeep/archive.db',
      lastSuccessfulBackupAt: '2026-05-18T14:23:00Z',
      warning: null,
    },
    appLockStatus: {} as AppSnapshot['appLockStatus'],
    keyringStatus: {} as AppSnapshot['keyringStatus'],
    aiStatus: {} as AppSnapshot['aiStatus'],
    intelligenceStatus: {} as AppSnapshot['intelligenceStatus'],
    browserProfiles: [],
    recentRuns: [],
    recentImportBatches: [],
  }
}

function makeDashboard(): DashboardSnapshot {
  return {
    generatedAt: '2026-05-19T00:00:00Z',
    totalProfiles: 1,
    totalUrls: 1000,
    totalVisits: 2500,
    totalDownloads: 0,
    lastSuccessfulBackupAt: '2026-05-18T14:23:00Z',
    recentRuns: [
      {
        id: 1,
        startedAt: '2026-05-18T14:00:00Z',
        finishedAt: '2026-05-18T14:23:00Z',
        status: 'completed',
        manifestHash: 'abcdef1234567890',
        profilesProcessed: 1,
        newVisits: 12,
        newUrls: 4,
        newDownloads: 0,
      },
    ],
    storage: {
      archiveDatabaseBytes: 8 * 1024 * 1024 * 1024,
      sourceEvidenceDatabaseBytes: 0,
      searchDatabaseBytes: 1 * 1024 * 1024 * 1024,
      intelligenceDatabaseBytes: 200 * 1024 * 1024,
      manifestBytes: 8 * 1024 * 1024,
      snapshotBytes: 800 * 1024 * 1024,
      exportBytes: 0,
      stagingBytes: 0,
      quarantineBytes: 0,
      semanticSidecarBytes: 0,
      intelligenceBlobBytes: 0,
    },
    nextAction: null,
  }
}
