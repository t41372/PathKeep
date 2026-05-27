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
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '@/app/shell-data-context'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import type * as CoreIntelligenceApi from '@/lib/core-intelligence/api'
import type * as ReactRouter from 'react-router-dom'
import { DashboardPage } from './index'
import type { AppSnapshot, DashboardSnapshot } from '@/lib/types'

const navigateMock = vi.fn()

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof ReactRouter>('react-router-dom')
  return {
    ...actual,
    useNavigate: () => navigateMock,
  }
})

vi.mock('@/lib/core-intelligence/api', () => ({
  getOnThisDay: vi.fn().mockResolvedValue({
    data: [],
    meta: {
      sectionId: 'on-this-day',
      window: { kind: 'calendar-day-history', referenceDate: '2026-05-20' },
      moduleIds: [],
      sourceTables: [],
      includesEnrichment: false,
      state: 'ready',
      notes: [],
    },
  }),
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
  beforeEach(async () => {
    navigateMock.mockReset()
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    vi.mocked(api.getOnThisDay).mockReset()
    vi.mocked(api.getOnThisDay).mockResolvedValue({
      data: [],
      meta: readyOnThisDayMeta(),
    })
    vi.mocked(api.getPathFlows).mockReset()
    vi.mocked(api.getPathFlows).mockResolvedValue({
      data: [],
      meta: { state: 'ready' },
    } as never)
    vi.mocked(api.getDiscoveryTrend).mockReset()
    vi.mocked(api.getDiscoveryTrend).mockResolvedValue({
      data: { points: [], availableYears: [] },
      meta: { state: 'ready' },
    } as never)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

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
      dashboard: { ...makeDashboard(), earliestVisitAt: null },
    })
    expect(screen.getByText('—')).toBeInTheDocument()
  })

  test('uses the current day as the archive span end when latest visit is missing', () => {
    vi.setSystemTime(new Date(2026, 4, 20, 12, 0, 0))
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: {
        ...makeDashboard(),
        earliestVisitAt: '2026-04-19T00:00:00Z',
        latestVisitAt: null,
      },
    })
    expect(screen.getByText('Span').previousElementSibling).toHaveTextContent(
      '1m',
    )
  })

  test('renders archive span, zero-size fallback, source count, and missing manifest state from read models', async () => {
    renderDashboard({
      snapshot: {
        ...makeSnapshot(),
        browserProfiles: [
          { profileId: 'chrome:Default' },
          { profileId: 'firefox:default' },
        ],
        archiveStatus: {
          ...makeSnapshot().archiveStatus,
          databasePath: '',
        },
        config: {
          ...makeSnapshot().config,
          archiveMode: 'Encrypted',
        },
      } as AppSnapshot,
      dashboard: {
        ...makeDashboard(),
        earliestVisitAt: '2026-05-18T00:00:00Z',
        latestVisitAt: '2026-05-18T12:00:00Z',
        recentRuns: [{ ...makeDashboard().recentRuns[0], manifestHash: null }],
        storage: zeroStorage(),
      },
    })

    expect(
      await screen.findByText('No archived pages exactly one year ago.'),
    ).toBeInTheDocument()
    expect(
      await screen.findByTestId('dashboard-active-threads-empty'),
    ).toBeInTheDocument()

    const archiveCard = screen.getByTestId('dashboard-archive-card')
    expect(screen.getByText('today')).toBeInTheDocument()
    expect(screen.getAllByText('0 B').length).toBeGreaterThan(0)
    expect(
      screen.getByText('Sources').previousElementSibling,
    ).toHaveTextContent('2')
    expect(archiveCard).toHaveTextContent('Encrypted')
    expect(archiveCard).toHaveTextContent('Awaiting first run')
    expect(archiveCard).toHaveTextContent('----…----')
    expect(archiveCard).toHaveTextContent('~/PathKeep/archive.db')
  })

  test('on-this-day null data renders the empty state instead of stale entries', async () => {
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    vi.mocked(api.getOnThisDay).mockResolvedValueOnce({
      data: null,
      meta: readyOnThisDayMeta(),
    } as never)
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    await waitFor(() => expect(api.getOnThisDay).toHaveBeenCalled())
    expect(
      await screen.findByText('No archived pages exactly one year ago.'),
    ).toBeInTheDocument()
  })

  test('discarding a stale successful on-this-day response prevents late entries from appearing', async () => {
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    let resolveRequest:
      | ((
          value: Awaited<ReturnType<typeof CoreIntelligenceApi.getOnThisDay>>,
        ) => void)
      | undefined
    const pending = new Promise<
      Awaited<ReturnType<typeof CoreIntelligenceApi.getOnThisDay>>
    >((resolve) => {
      resolveRequest = resolve
    })
    vi.mocked(api.getOnThisDay).mockReturnValueOnce(pending)
    const { rerenderDashboard } = renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    await waitFor(() => expect(api.getOnThisDay).toHaveBeenCalled())

    rerenderDashboard({
      snapshot: { ...makeSnapshot(), config: { initialized: false } as never },
      dashboard: makeDashboard(),
    })
    resolveRequest?.({
      data: [
        {
          year: 2024,
          date: '2024-05-19',
          totalVisits: 7,
          deepDiveSessions: 1,
          topDomains: ['github.com'],
          summary: 'Stale summary',
        },
      ],
      meta: readyOnThisDayMeta(),
    })
    await pending

    expect(screen.queryByText('Stale summary')).not.toBeInTheDocument()
  })

  test('discarding a stale failed on-this-day response prevents late errors from appearing', async () => {
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    let rejectRequest: ((error: Error) => void) | undefined
    const pending = new Promise<never>((_, reject) => {
      rejectRequest = reject
    })
    vi.mocked(api.getOnThisDay).mockReturnValueOnce(pending)
    const { rerenderDashboard } = renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    await waitFor(() => expect(api.getOnThisDay).toHaveBeenCalled())

    rerenderDashboard({
      snapshot: { ...makeSnapshot(), config: { initialized: false } as never },
      dashboard: makeDashboard(),
    })
    rejectRequest?.(new Error('late failure'))
    await pending.catch(() => undefined)

    expect(
      screen.queryByText('Could not load entries for this day.'),
    ).not.toBeInTheDocument()
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

  test('clicking the On This Day entry fires the route-level openEntry + jumpToDate handlers', async () => {
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
      meta: readyOnThisDayMeta(),
    })

    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    // The entry button is the only path that fires the route's onOpenEntry
    // callback (line 156-160). The card displays the summary as its title.
    await screen.findByText('A year ago summary')
    await user.click(screen.getByText('A year ago summary'))
    expect(navigateMock).toHaveBeenLastCalledWith(
      '/explorer?date=2024-05-19&source=on-this-day',
    )
    // The card's header right-slot button fires onJumpToDate (line 152-155);
    // its label is the localised "month day" target string.
    const headerButton = screen
      .getByTestId('dashboard-on-this-day')
      .querySelector('header button')
    if (!headerButton) throw new Error('on-this-day jumpToDate trigger missing')
    const targetDate = new Date()
    targetDate.setFullYear(targetDate.getFullYear() - 1)
    const targetIso = targetDate.toISOString().slice(0, 10)
    await user.click(headerButton)
    expect(navigateMock).toHaveBeenLastCalledWith(
      `/explorer?date=${encodeURIComponent(targetIso)}&source=on-this-day`,
    )
  })

  test('"Insights" badge in year heatmap card navigates to /intelligence', async () => {
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    const badges = screen.getAllByText(/Insights/)
    await user.click(badges[0])
    expect(navigateMock).toHaveBeenLastCalledWith('/intelligence')
  })

  test('Active threads "All threads" badge navigates to /intelligence', async () => {
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    await user.click(screen.getByText(/All threads/))
    expect(navigateMock).toHaveBeenLastCalledWith('/intelligence')
  })

  test('clicking an Active Threads row fires the route onOpenThread navigate', async () => {
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    vi.mocked(api.getPathFlows).mockResolvedValueOnce({
      data: [
        {
          flowId: 'flow-1',
          stepCount: 3,
          occurrenceCount: 4,
          steps: [
            { index: 0, label: 'github.com' },
            { index: 1, label: 'docs.rs' },
            { index: 2, label: 'crates.io' },
          ],
        },
      ],
      meta: { state: 'ready' },
    } as never)
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    const row = await screen.findByTestId('dashboard-active-threads-row-flow-1')
    await user.click(row)
    expect(navigateMock).toHaveBeenLastCalledWith('/intelligence')
  })

  test('renders the morning greeting branch when hour is before noon', () => {
    vi.setSystemTime(new Date(2026, 4, 20, 9, 0, 0))
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    expect(screen.getByText('Good morning')).toBeInTheDocument()
  })

  test('renders the afternoon greeting branch when hour falls between 12 and 18', () => {
    // Pin time to 14:00 local so useMemoGreeting takes the `hour < 18` arm.
    vi.setSystemTime(new Date(2026, 4, 20, 14, 0, 0))
    try {
      renderDashboard({
        snapshot: makeSnapshot(),
        dashboard: makeDashboard(),
      })
      expect(screen.getByText('Good afternoon')).toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }
  })

  test('renders the evening greeting branch when hour is >= 18', () => {
    vi.setSystemTime(new Date(2026, 4, 20, 21, 0, 0))
    try {
      renderDashboard({
        snapshot: makeSnapshot(),
        dashboard: makeDashboard(),
      })
      expect(screen.getByText('Good evening')).toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }
  })

  test('clicking a Year Heatmap day cell fires the route onSelectDate navigate', async () => {
    const api = await vi.importMock<typeof CoreIntelligenceApi>(
      '@/lib/core-intelligence/api',
    )
    vi.mocked(api.getDiscoveryTrend).mockResolvedValueOnce({
      data: {
        points: [{ dateKey: '2026-04-20', totalVisits: 8 }],
        availableYears: [2026],
      },
      meta: { state: 'ready' },
    } as never)
    renderDashboard({
      snapshot: makeSnapshot(),
      dashboard: makeDashboard(),
    })
    const user = userEvent.setup()
    const cell = await screen.findByRole('button', {
      name: /2026-04-20/,
    })
    await user.click(cell)
    expect(navigateMock).toHaveBeenLastCalledWith(
      '/explorer?date=2026-04-20&source=on-this-day',
    )
  })
})

function renderDashboard(overrides: Partial<ShellDataContextValue>) {
  const view = render(dashboardTree(overrides))
  return {
    ...view,
    rerenderDashboard: (nextOverrides: Partial<ShellDataContextValue>) => {
      view.rerender(dashboardTree(nextOverrides))
    },
  }
}

function dashboardTree(overrides: Partial<ShellDataContextValue>) {
  return (
    <I18nProvider>
      <ProfileScopeProvider>
        <MemoryRouter>
          <ShellDataContext.Provider value={makeShellValue(overrides)}>
            <DashboardPage />
          </ShellDataContext.Provider>
        </MemoryRouter>
      </ProfileScopeProvider>
    </I18nProvider>
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

function readyOnThisDayMeta(): Awaited<
  ReturnType<typeof CoreIntelligenceApi.getOnThisDay>
>['meta'] {
  return {
    sectionId: 'on-this-day',
    window: { kind: 'calendar-day-history', referenceDate: '2026-05-20' },
    moduleIds: [],
    sourceTables: [],
    includesEnrichment: false,
    state: 'ready',
    notes: [],
  }
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

function zeroStorage(): DashboardSnapshot['storage'] {
  return {
    archiveDatabaseBytes: 0,
    sourceEvidenceDatabaseBytes: 0,
    searchDatabaseBytes: 0,
    intelligenceDatabaseBytes: 0,
    manifestBytes: 0,
    snapshotBytes: 0,
    exportBytes: 0,
    stagingBytes: 0,
    quarantineBytes: 0,
    semanticSidecarBytes: 0,
    intelligenceBlobBytes: 0,
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
