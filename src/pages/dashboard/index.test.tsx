/**
 * @file index.test.tsx
 * @description Route-level coverage for Dashboard data gating and next-action copy.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Verify DashboardPage chooses zero-state versus ready panels from shell data.
 * - Protect On This Day success/error handling and next-action localization.
 *
 * ## Not responsible for
 * - Re-testing every dashboard panel's layout.
 * - Re-testing shell bootstrap providers.
 *
 * ## Dependencies
 * - Wraps DashboardPage in the same shell, profile-scope, router, and i18n contexts it reads.
 * - Mocks panel children so this suite stays focused on route orchestration.
 *
 * ## Performance notes
 * - Fixtures are small clones of the preview state, so no route test walks real archive data.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import { createMockState } from '../../lib/backend-preview-state'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { I18nProvider } from '../../lib/i18n'
import { ProfileScopeContext } from '../../lib/profile-scope-context'
import type {
  AppSnapshot,
  BrowserProfile,
  BackupRunOverview,
  DashboardSnapshot,
  StorageSummary,
} from '../../lib/types'
import { DashboardPage } from './index'

vi.mock('../../components/primitives/status-callout', () => ({
  StatusCallout: ({
    body,
    eyebrow,
    title,
  }: {
    body?: string
    eyebrow?: string
    title: string
  }) => (
    <div>
      {eyebrow ? <span>{eyebrow}</span> : null}
      <strong>{title}</strong>
      {body ? <p>{body}</p> : null}
    </div>
  ),
}))

vi.mock('./zero-state', () => ({
  DashboardZeroState: ({
    snapshotInitialized,
  }: {
    snapshotInitialized: boolean
  }) => (
    <div data-testid="dashboard-zero-state">
      {snapshotInitialized ? 'initialized' : 'not initialized'}
    </div>
  ),
}))

vi.mock('./panels', () => ({
  DashboardArchiveBoundaryPanel: () => <div>archive boundary</div>,
  DashboardIntelligencePanel: ({
    backgroundQueueCount,
  }: {
    backgroundQueueCount: number | null
  }) => <div>queue:{backgroundQueueCount ?? 'none'}</div>,
  DashboardOnThisDayPanel: ({
    activeOnThisDay,
    activeOnThisDayError,
    onThisDayLoading,
  }: {
    activeOnThisDay: Array<{ summary?: string | null }>
    activeOnThisDayError: string | null
    onThisDayLoading: boolean
  }) => (
    <div>
      <span>today-loading:{String(onThisDayLoading)}</span>
      <span>today-error:{activeOnThisDayError ?? 'none'}</span>
      {activeOnThisDay.map((entry) => (
        <span key={entry.summary ?? 'entry'}>{entry.summary}</span>
      ))}
    </div>
  ),
  DashboardRecentRunsPanel: () => <div>recent runs</div>,
  DashboardRhythmPanel: () => <div>rhythm</div>,
  DashboardStatsRow: () => <div>stats row</div>,
  DashboardStorageFootprintPanel: () => <div>storage footprint</div>,
  DashboardTrustActionsPanel: () => <div>trust actions</div>,
}))

describe('DashboardPage', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('renders zero state and skips On This Day loading before initialization', () => {
    const snapshot = snapshotFixture({ initialized: false })
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue({
      data: [],
      meta: onThisDayMeta(),
    })

    renderDashboard({
      dashboard: dashboardFixture({ recentRuns: [] }),
      snapshot,
    })

    expect(screen.getByTestId('dashboard-zero-state')).toHaveTextContent(
      'not initialized',
    )
    expect(coreIntelligenceApi.getOnThisDay).not.toHaveBeenCalled()
  })

  test('loads On This Day and localizes initialization next actions', async () => {
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue({
      data: [
        {
          date: '2026-04-25',
          deepDiveSessions: 1,
          summary: 'Read about local-first storage',
          topDomains: ['sqlite.org'],
          totalVisits: 12,
          year: 2025,
        },
      ],
      meta: onThisDayMeta(),
    })

    renderDashboard({
      dashboard: dashboardFixture({
        nextAction: 'Initialize the archive before running backups.',
      }),
      snapshot: snapshotFixture({ initialized: true }),
    })

    expect(
      screen.getByText(
        'Initialize the archive before running your first manual backup.',
      ),
    ).toBeVisible()
    expect(
      await screen.findByText('Read about local-first storage'),
    ).toBeVisible()
    expect(screen.getByText('today-error:none')).toBeVisible()
  })

  test('surfaces On This Day errors and localizes first-backup next actions', async () => {
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockRejectedValue(
      new Error('on-this-day failed'),
    )

    renderDashboard({
      dashboard: dashboardFixture({
        nextAction:
          'Run a manual backup to create the first manifest and snapshot artifacts.',
      }),
      snapshot: snapshotFixture({ initialized: true }),
    })

    expect(
      screen.getByText(
        'Run a manual backup to create the first manifest and snapshot artifacts.',
      ),
    ).toBeVisible()
    expect(
      await screen.findByText('today-error:on-this-day failed'),
    ).toBeVisible()
  })

  test('preserves unknown dashboard next-action copy as backend-authored text', () => {
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue({
      data: [],
      meta: onThisDayMeta(),
    })

    renderDashboard({
      dashboard: dashboardFixture({
        nextAction: 'Review browser permissions before the next backup.',
      }),
      snapshot: snapshotFixture({ initialized: true }),
    })

    expect(
      screen.getByText('Review browser permissions before the next backup.'),
    ).toBeVisible()
  })

  test('renders active profile scope in zero state', () => {
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue({
      data: [],
      meta: onThisDayMeta(),
    })

    renderDashboard({
      activeProfileId: 'chrome:Default',
      dashboard: dashboardFixture({ recentRuns: [] }),
      snapshot: snapshotFixture({ initialized: true }),
    })

    expect(screen.getByText('Profile scope')).toBeVisible()
    expect(screen.getByText('Default')).toBeVisible()
    expect(screen.getByTestId('dashboard-zero-state')).toHaveTextContent(
      'initialized',
    )
  })

  test('renders security and Safari callouts with missing background queues', async () => {
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue({
      data: [],
      meta: onThisDayMeta(),
    })
    const snapshot = snapshotFixture({ initialized: true })
    const safariProfile = safariProfileFixture()

    renderDashboard({
      activeProfileId: 'safari:Personal',
      dashboard: dashboardFixture(),
      shellOverrides: {
        runtimeStatus: {
          aiQueue: null,
          error: null,
          intelligence: null,
          loading: false,
        },
      },
      snapshot: {
        ...snapshot,
        browserProfiles: [safariProfile],
        config: {
          ...snapshot.config,
          archiveMode: 'Encrypted',
          rememberDatabaseKeyInKeyring: true,
          selectedProfileIds: [safariProfile.profileId],
        },
        keyringStatus: {
          ...snapshot.keyringStatus,
          storedSecret: false,
        },
      },
    })

    expect(screen.getByText('Personal')).toBeVisible()
    expect(screen.getByText('System keychain not available')).toBeVisible()
    expect(screen.getByText('Safari needs Full Disk Access')).toBeVisible()
    expect(await screen.findByText('today-error:none')).toBeVisible()
  })

  test('renders each dashboard repair callout independently', () => {
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue({
      data: [],
      meta: onThisDayMeta(),
    })
    const snapshot = snapshotFixture({ initialized: true })
    const safariProfile = safariProfileFixture()

    const safariOnly = renderDashboard({
      activeProfileId: 'safari:Personal',
      dashboard: dashboardFixture(),
      snapshot: {
        ...snapshot,
        browserProfiles: [safariProfile],
        config: {
          ...snapshot.config,
          archiveMode: 'Plaintext',
          rememberDatabaseKeyInKeyring: false,
          selectedProfileIds: [safariProfile.profileId],
        },
        keyringStatus: {
          ...snapshot.keyringStatus,
          storedSecret: false,
        },
      },
    })
    expect(screen.queryByText('System keychain not available')).toBeNull()
    expect(screen.getByText('Safari needs Full Disk Access')).toBeVisible()
    safariOnly.unmount()

    renderDashboard({
      dashboard: dashboardFixture(),
      snapshot: {
        ...snapshot,
        browserProfiles: [],
        config: {
          ...snapshot.config,
          archiveMode: 'Encrypted',
          rememberDatabaseKeyInKeyring: true,
          selectedProfileIds: [],
        },
        keyringStatus: {
          ...snapshot.keyringStatus,
          storedSecret: false,
        },
      },
    })
    expect(screen.getByText('System keychain not available')).toBeVisible()
    expect(screen.queryByText('Safari needs Full Disk Access')).toBeNull()
  })

  test('uses fallback copy for non-error On This Day failures and ignores late completions', async () => {
    const onThisDay =
      deferred<Awaited<ReturnType<typeof coreIntelligenceApi.getOnThisDay>>>()
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay')
      .mockRejectedValueOnce('offline')
      .mockReturnValueOnce(onThisDay.promise)

    const dashboard = dashboardFixture()
    const snapshot = snapshotFixture({ initialized: true })
    const firstRender = renderDashboard({ dashboard, snapshot })

    expect(
      await screen.findByText(
        'today-error:No history found for this date in past years.',
      ),
    ).toBeVisible()
    firstRender.unmount()

    const secondRender = renderDashboard({
      dashboard,
      snapshot,
      shellOverrides: { refreshKey: 2 },
    })
    secondRender.unmount()
    await onThisDay.resolve({
      data: [
        {
          date: '2026-04-25',
          deepDiveSessions: 0,
          summary: 'Late entry',
          topDomains: [],
          totalVisits: 1,
          year: 2024,
        },
      ],
      meta: onThisDayMeta(),
    })
  })

  test('handles empty On This Day payloads and late rejected loads without callout noise', async () => {
    const lateFailure =
      deferred<Awaited<ReturnType<typeof coreIntelligenceApi.getOnThisDay>>>()
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay')
      .mockResolvedValueOnce({
        data: null,
        meta: onThisDayMeta(),
      } as unknown as Awaited<
        ReturnType<typeof coreIntelligenceApi.getOnThisDay>
      >)
      .mockReturnValueOnce(lateFailure.promise)

    const snapshot = snapshotFixture({ initialized: true })
    const dashboard = dashboardFixture()
    const firstRender = renderDashboard({
      dashboard,
      snapshot: {
        ...snapshot,
        browserProfiles: [],
        config: {
          ...snapshot.config,
          archiveMode: 'Plaintext',
          rememberDatabaseKeyInKeyring: false,
          selectedProfileIds: [],
        },
        keyringStatus: {
          ...snapshot.keyringStatus,
          storedSecret: false,
        },
      },
    })

    expect(await screen.findByText('today-error:none')).toBeVisible()
    expect(screen.queryByText('System keychain not available')).toBeNull()
    expect(screen.queryByText('Safari needs Full Disk Access')).toBeNull()
    firstRender.unmount()

    const secondRender = renderDashboard({ dashboard, snapshot })
    secondRender.unmount()
    lateFailure.reject(new Error('late failure'))
    await Promise.resolve()
  })
})

function renderDashboard({
  activeProfileId = null,
  dashboard,
  shellOverrides = {},
  snapshot,
}: {
  activeProfileId?: string | null
  dashboard: DashboardSnapshot | null
  shellOverrides?: Partial<ShellDataContextValue>
  snapshot: AppSnapshot | null
}) {
  return render(
    <MemoryRouter>
      <I18nProvider>
        <ProfileScopeContext.Provider
          value={{ activeProfileId, setActiveProfileId: vi.fn() }}
        >
          <ShellDataContext.Provider
            value={{
              ...shellValue({ dashboard, snapshot }),
              ...shellOverrides,
            }}
          >
            <DashboardPage />
          </ShellDataContext.Provider>
        </ProfileScopeContext.Provider>
      </I18nProvider>
    </MemoryRouter>,
  )
}

function shellValue({
  dashboard,
  snapshot,
}: {
  dashboard: DashboardSnapshot | null
  snapshot: AppSnapshot | null
}): ShellDataContextValue {
  return {
    appLockStatus: snapshot?.appLockStatus ?? null,
    buildInfo: null,
    busyAction: null,
    busyOverlay: null,
    clearAppLockPasscode: vi.fn(),
    clearNotice: vi.fn(),
    dashboard,
    dashboardLoading: false,
    error: null,
    initializeArchive: vi.fn(),
    loading: false,
    lockAppSession: vi.fn(),
    notice: null,
    refreshAppData: vi.fn(),
    refreshKey: 1,
    refreshRuntimeStatus: vi.fn(),
    runBackup: vi.fn(),
    runtimeStatus: {
      aiQueue: {
        failed: 1,
        paused: false,
        queued: 2,
        running: 3,
      },
      error: null,
      intelligence: {
        generatedAt: '2026-04-25T12:00:00Z',
        modules: [],
        queue: {
          failed: 1,
          queued: 1,
          running: 1,
        },
      },
      loading: false,
    },
    saveConfig: vi.fn(),
    setAppLockPasscode: vi.fn(),
    snapshot,
    unlockAppSession: vi.fn(),
  } as unknown as ShellDataContextValue
}

function snapshotFixture({
  initialized,
}: {
  initialized: boolean
}): AppSnapshot {
  const state = createMockState()
  return {
    ...state.snapshot,
    config: {
      ...state.snapshot.config,
      initialized,
      selectedProfileIds: ['chrome:Default'],
    },
    recentRuns: [runFixture()],
  }
}

function dashboardFixture(
  overrides: Partial<DashboardSnapshot> = {},
): DashboardSnapshot {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    lastSuccessfulBackupAt: '2026-04-25T11:00:00Z',
    nextAction: null,
    recentRuns: [runFixture()],
    storage: storageFixture(),
    totalDownloads: 1,
    totalProfiles: 1,
    totalUrls: 8,
    totalVisits: 12,
    ...overrides,
  }
}

function runFixture(): BackupRunOverview {
  return {
    finishedAt: '2026-04-25T11:05:00Z',
    id: 1,
    manifestHash: 'sha256:run',
    newDownloads: 0,
    newUrls: 4,
    newVisits: 6,
    profileScope: ['chrome:Default'],
    profilesProcessed: 1,
    runType: 'backup',
    startedAt: '2026-04-25T11:00:00Z',
    status: 'success',
    trigger: 'manual',
  }
}

function storageFixture(): StorageSummary {
  return {
    archiveDatabaseBytes: 1,
    exportBytes: 0,
    intelligenceBlobBytes: 0,
    intelligenceDatabaseBytes: 0,
    manifestBytes: 1,
    quarantineBytes: 0,
    searchDatabaseBytes: 0,
    semanticSidecarBytes: 0,
    snapshotBytes: 1,
    sourceEvidenceDatabaseBytes: 1,
    stagingBytes: 0,
  }
}

function safariProfileFixture(): BrowserProfile {
  return {
    accessIssue: 'Full Disk Access is required.',
    browserFamily: 'safari',
    browserName: 'Safari',
    faviconsBytes: 0,
    faviconsPath: null,
    historyBytes: 1024,
    historyExists: true,
    historyFileName: 'History.db',
    historyPath: '/Users/test/Library/Safari/History.db',
    historyReadable: false,
    profileId: 'safari:Personal',
    profileName: 'Personal',
    profilePath: '/Users/test/Library/Safari',
    retentionBoundary: { kind: 'macos-safari', localDays: null },
    supportingBytes: 0,
    userName: 'test',
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve
    reject = nextReject
  })
  return {
    promise,
    reject,
    resolve: (value: T) => {
      resolve(value)
      return promise
    },
  }
}

function onThisDayMeta() {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    includesEnrichment: false,
    moduleIds: ['on-this-day'],
    notes: [],
    sectionId: 'on-this-day',
    sourceTables: ['daily_rollups'],
    state: 'ready' as const,
    stateReason: null,
    window: {
      dateRange: { start: '2026-04-25', end: '2026-04-25' },
      kind: 'date-range' as const,
    },
  }
}
