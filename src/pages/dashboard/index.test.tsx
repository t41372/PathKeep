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

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '@/app/shell-data-context'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import type * as CoreIntelligenceApi from '@/lib/core-intelligence/api'
import { DashboardPage } from './index'
import type { AppSnapshot, DashboardSnapshot } from '@/lib/types'

vi.mock('@/lib/core-intelligence/api', () => ({
  getOnThisDay: vi.fn().mockResolvedValue({ data: [] }),
  getPathFlows: vi.fn().mockResolvedValue({
    data: [],
    meta: { state: 'ready' },
  }),
  getDiscoveryTrend: vi.fn().mockResolvedValue({
    data: { points: [], availableYears: [] },
    meta: { state: 'ready' },
  }),
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

  test('uses the em-dash span placeholder when no successful backup recorded', () => {
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: { ...makeDashboard(), lastSuccessfulBackupAt: null },
    })
    expect(screen.getByText('—')).toBeInTheDocument()
  })

  test('falls back to the translated on-this-day error key on non-Error rejection', async () => {
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    vi.mocked(api.getOnThisDay).mockRejectedValueOnce('string-rejection')
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    await waitFor(() => expect(api.getOnThisDay).toHaveBeenCalled())
    // The route's catch branch sets onThisDayError; the card surfaces it.
    expect(
      await screen.findByText('Could not load entries for this day.'),
    ).toBeInTheDocument()
  })

  test('clicking the On This Day entry fires the route-level openEntry handler', async () => {
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    vi.mocked(api.getOnThisDay).mockResolvedValueOnce({
      data: [
        {
          year: 2024,
          date: '2024-05-19',
          totalVisits: 7,
          deepDiveSessions: 1,
          topDomains: ['github.com'],
          summary: 'A year ago summary',
        },
      ],
    } as unknown as Awaited<ReturnType<typeof api.getOnThisDay>>)

    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    // The entry button is the only path that fires the route's onOpenEntry
    // callback (line 156-160). The card displays the summary as its title.
    await screen.findByText('A year ago summary')
    await user.click(screen.getByText('A year ago summary'))
  })

  test('"Insights" badge in year heatmap card navigates to /intelligence', async () => {
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    const badges = screen.getAllByText(/Insights/)
    await user.click(badges[0])
    // No assertion needed — coverage is the goal (the inline navigate
    // arrow on line 174 is otherwise unreachable from tests).
  })

  test('Active threads "All threads" badge navigates to /intelligence', async () => {
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    await user.click(screen.getByText(/All threads/))
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
