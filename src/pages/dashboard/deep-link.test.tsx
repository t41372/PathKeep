/**
 * Integration test for the dashboard Active Threads deep-link.
 *
 * The unit tests in `index.test.tsx` mock `useNavigate` and assert the computed
 * URL string. That alone gives false confidence: an Active Threads click could
 * produce a tidy URL yet land somewhere that renders nothing different from
 * "See all". This test wires the real dashboard and the real
 * `/intelligence/domain/:domain` route together (no `useNavigate` mock), clicks
 * a thread row, and asserts the destination genuinely surfaces the focused path
 * flow — the route's "Focused path flow" callout appears.
 *
 * Out of scope:
 * - Backend correctness of path flows / domain deep-dive (own module tests).
 * - The This Week / On This Day / heatmap cards (own tests).
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '@/app/shell-data-context'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import type {
  CoreIntelligenceSectionMeta,
  DomainDeepDive,
  PathFlow,
} from '@/lib/core-intelligence'
import type { AppSnapshot, DashboardSnapshot } from '@/lib/types'
import { DashboardPage } from './index'
import { DomainDeepDiveRoutePage } from '../intelligence'

const {
  getCompareSetDetailMock,
  getDiscoveryTrendMock,
  getDomainDeepDiveMock,
  getOnThisDayMock,
  getPathFlowsMock,
  getSearchQueriesMock,
  peekPathFlowsMock,
  peekSearchQueriesMock,
} = vi.hoisted(() => ({
  getCompareSetDetailMock: vi.fn(),
  getDiscoveryTrendMock: vi.fn(),
  getDomainDeepDiveMock: vi.fn(),
  getOnThisDayMock: vi.fn(),
  getPathFlowsMock: vi.fn(),
  getSearchQueriesMock: vi.fn(),
  peekPathFlowsMock: vi.fn(),
  peekSearchQueriesMock: vi.fn(),
}))

vi.mock('@/lib/core-intelligence/api', () => ({
  getCompareSetDetail: getCompareSetDetailMock,
  getDiscoveryTrend: getDiscoveryTrendMock,
  getDomainDeepDive: getDomainDeepDiveMock,
  getOnThisDay: getOnThisDayMock,
  getPathFlows: getPathFlowsMock,
  getSearchQueries: getSearchQueriesMock,
  peekPathFlows: peekPathFlowsMock,
  peekSearchQueries: peekSearchQueriesMock,
}))

// The path-flow whose row the user clicks. Its first registrable-domain step is
// `github.com`, so the deep-link must open `/intelligence/domain/github.com`
// carrying the path-flow focus, and the focused-flow callout must render there
// because `github.com` is one of the flow's steps.
const FOCUS_FLOW: PathFlow = {
  flowId: 'path-flow:github:3:abc',
  flowPattern: 'github.com → docs.rs → crates.io',
  stepCount: 3,
  occurrenceCount: 6,
  lastSeenAt: '2026-05-20T00:00:00Z',
  steps: [
    { index: 0, label: 'github.com', registrableDomain: 'github.com' },
    { index: 1, label: 'docs.rs', registrableDomain: 'docs.rs' },
    { index: 2, label: 'crates.io', registrableDomain: 'crates.io' },
  ],
}

describe('Dashboard Active Threads deep-link (integration)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getOnThisDayMock.mockResolvedValue({ data: [], meta: onThisDayMeta() })
    getDiscoveryTrendMock.mockResolvedValue({
      data: { points: [], availableYears: [] },
      meta: { state: 'ready' },
    })
    // Both the dashboard card and the domain route read getPathFlows; returning
    // the focus flow for both keeps the focused-flow lookup satisfied.
    getPathFlowsMock.mockResolvedValue({
      data: [FOCUS_FLOW],
      meta: dateRangeMeta(),
    })
    getCompareSetDetailMock.mockResolvedValue(null)
    getDomainDeepDiveMock.mockResolvedValue({
      data: domainFixture(),
      meta: dateRangeMeta(),
    })
    getSearchQueriesMock.mockResolvedValue({
      data: { families: [], totalQueries: 0 },
      meta: dateRangeMeta(),
    })
    peekSearchQueriesMock.mockReturnValue(null)
    peekPathFlowsMock.mockReturnValue(null)
  })

  test('clicking a thread row lands on a destination that renders the focused flow', async () => {
    const user = userEvent.setup()
    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <MemoryRouter initialEntries={['/']}>
            <ShellDataContext.Provider value={makeShellValue()}>
              <Routes>
                <Route path="/" element={<DashboardPage />} />
                <Route
                  path="/intelligence/domain/:domain"
                  element={<DomainDeepDiveRoutePage />}
                />
              </Routes>
            </ShellDataContext.Provider>
          </MemoryRouter>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    const row = await screen.findByTestId(
      'dashboard-active-threads-row-path-flow:github:3:abc',
    )
    await user.click(row)

    // We landed on the domain deep-dive (not the generic overview)...
    const page = await screen.findByTestId('domain-deep-dive-page')
    expect(page).toBeInTheDocument()
    // ...and that route actually surfaces the focused flow with its callout,
    // proving the deep-link is distinguishable from "See all".
    expect(await screen.findByText('Focused path flow')).toBeVisible()
    // The domain route fetched flows for the focus id's step count (3).
    await waitFor(() =>
      expect(getDomainDeepDiveMock).toHaveBeenCalledWith(
        'github.com',
        expect.anything(),
        null,
      ),
    )
  })
})

function makeShellValue(): ShellDataContextValue {
  return {
    buildInfo: null,
    appLockStatus: null,
    snapshot: makeSnapshot(),
    dashboard: makeDashboard(),
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
    errorKind: null,
    clearError: vi.fn(),
    recovery: null,
    runFullArchiveRestore: vi.fn().mockResolvedValue({}),
  } as ShellDataContextValue
}

function onThisDayMeta(): CoreIntelligenceSectionMeta {
  return {
    sectionId: 'on-this-day',
    generatedAt: '2026-05-20T00:00:00Z',
    window: { kind: 'calendar-day-history', referenceDate: '2026-05-20' },
    moduleIds: [],
    sourceTables: [],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function dateRangeMeta(): CoreIntelligenceSectionMeta {
  return {
    sectionId: 'domain-deep-dive',
    generatedAt: '2026-05-20T00:00:00Z',
    window: {
      kind: 'date-range',
      dateRange: { start: '2026-04-20', end: '2026-05-20' },
    },
    moduleIds: ['domain-deep-dive'],
    sourceTables: ['domain_daily_rollups'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function domainFixture(): DomainDeepDive {
  return {
    registrableDomain: 'github.com',
    displayName: 'GitHub',
    domainCategory: 'reference',
    totalVisits: 1200,
    activeDays: 12,
    trailCount: 4,
    arrivalBreakdown: { search: 0, link: 0, typed: 0, other: 0 },
    topPages: [],
    topReferrers: [],
    topExits: [],
    visitTrend: [],
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

function makeDashboard(): DashboardSnapshot {
  return {
    generatedAt: '2026-05-19T00:00:00Z',
    totalProfiles: 1,
    totalUrls: 1000,
    totalVisits: 2500,
    totalDownloads: 0,
    lastSuccessfulBackupAt: '2026-05-18T14:23:00Z',
    recentRuns: [],
    storage: {
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
    },
    nextAction: null,
  }
}
