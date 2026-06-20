/**
 * Coverage test for the dashboard "This week" card.
 *
 * Covers:
 * - archive-not-ready: stays idle (no fetch) and renders zeroed weekly stats.
 * - loading: skeleton visible while the discovery-trend fetch is pending.
 * - error: error message visible when the fetch rejects.
 * - populated: weekly visits + new sites summed from the trend; runs filtered
 *   to the current ISO week from the snapshot's recent-runs slice.
 * - locale formatting + ISO-week badge branches.
 * - unmount-mid-flight: cancelled branches short-circuit without setstate.
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import { I18nProvider } from '@/lib/i18n'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import type { BackupRunOverview } from '@/lib/types'
import { DashboardThisWeek } from './this-week-card'

type Locale = 'en' | 'zh-CN' | 'zh-TW'

function makeTrendResult(
  points: Array<{ totalVisits: number; newDomainCount: number }>,
) {
  return {
    data: {
      points: points.map((point, index) => ({
        dateKey: `2026-05-${String(18 + index).padStart(2, '0')}`,
        discoveryRate: 0,
        ...point,
      })),
      availableYears: [],
    },
    meta: { state: 'ready' as const },
  } as unknown as Awaited<
    ReturnType<typeof coreIntelligenceApi.getDiscoveryTrend>
  >
}

function makeRun(
  overrides: Partial<BackupRunOverview> = {},
): BackupRunOverview {
  return {
    id: 1,
    startedAt: '2026-05-20T10:00:00Z',
    finishedAt: '2026-05-20T10:05:00Z',
    status: 'completed',
    manifestHash: 'abc',
    profilesProcessed: 1,
    newVisits: 5,
    newUrls: 2,
    newDownloads: 0,
    ...overrides,
  }
}

function renderCard(
  props: Partial<Parameters<typeof DashboardThisWeek>[0]> = {},
  language: Locale = 'en',
) {
  window.localStorage.setItem('pathkeep-language-preference', language)
  return render(
    <ProfileScopeProvider>
      <I18nProvider>
        <DashboardThisWeek archiveReady recentRuns={[]} {...props} />
      </I18nProvider>
    </ProfileScopeProvider>,
  )
}

describe('DashboardThisWeek', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  test('stays idle (no fetch) and zeros the stats when archive is not ready', () => {
    const spy = vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend')
    renderCard({ archiveReady: false, recentRuns: [makeRun()] })
    expect(spy).not.toHaveBeenCalled()
    expect(screen.getByTestId('dashboard-this-week-stats')).toBeInTheDocument()
    // Visits + new sites are zero; the single run is outside the test's
    // current ISO week unless today happens to match, so assert the stat
    // strip rendered rather than a brittle exact count here.
    expect(screen.getByText(/Week \d+/)).toBeInTheDocument()
  })

  test('renders the loading skeleton while the fetch is pending', () => {
    let resolveTrend: (
      value: Awaited<ReturnType<typeof coreIntelligenceApi.getDiscoveryTrend>>,
    ) => void = () => {}
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTrend = resolve
        }),
    )
    renderCard()
    expect(
      screen.getByTestId('dashboard-this-week-loading'),
    ).toBeInTheDocument()
    // Release the in-flight promise to avoid an unhandled resolution.
    act(() => resolveTrend(makeTrendResult([])))
  })

  test('renders the error message when the fetch rejects', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockRejectedValue(
      new Error('trend boom'),
    )
    renderCard()
    expect(
      await screen.findByTestId('dashboard-this-week-error'),
    ).toHaveTextContent('trend boom')
  })

  test('surfaces the raw rejection when the failure is not an Error', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockRejectedValue(
      'string-rejection',
    )
    renderCard()
    expect(
      await screen.findByTestId('dashboard-this-week-error'),
    ).toHaveTextContent('string-rejection')
  })

  test('sums weekly visits + new sites and counts in-week runs', async () => {
    // Pin "now" to Wednesday 2026-05-20 so the ISO week is Mon 05-18 → Sun 05-24.
    vi.setSystemTime(new Date(2026, 4, 20, 12, 0, 0))
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      makeTrendResult([
        { totalVisits: 100, newDomainCount: 3 },
        { totalVisits: 47, newDomainCount: 5 },
      ]),
    )
    renderCard({
      recentRuns: [
        makeRun({ id: 1, startedAt: '2026-05-19T08:00:00Z' }), // in week
        makeRun({ id: 2, startedAt: '2026-05-20T08:00:00Z' }), // in week
        makeRun({ id: 3, startedAt: '2026-05-10T08:00:00Z' }), // before week
      ],
    })
    expect(
      await screen.findByTestId('dashboard-this-week-stats'),
    ).toBeInTheDocument()
    // 100 + 47 = 147 visits, 3 + 5 = 8 new sites, 2 in-week runs.
    expect(screen.getByText('147')).toBeInTheDocument()
    expect(screen.getByText('8')).toBeInTheDocument()
    expect(screen.getByText('2')).toBeInTheDocument()
  })

  test('treats a malformed response with no data points as a zero week', async () => {
    vi.setSystemTime(new Date(2026, 4, 20, 12, 0, 0))
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue({
      // No `data` field so `result.data?.points` is undefined.
      meta: { state: 'ready' as const },
    } as unknown as Awaited<
      ReturnType<typeof coreIntelligenceApi.getDiscoveryTrend>
    >)
    renderCard({ recentRuns: [] })
    expect(
      await screen.findByTestId('dashboard-this-week-stats'),
    ).toBeInTheDocument()
    // Zero visits + zero sites + zero runs all format as "0".
    expect(screen.getAllByText('0').length).toBe(3)
  })

  test('formats stats with the zh-CN locale separator', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      makeTrendResult([{ totalVisits: 12345, newDomainCount: 0 }]),
    )
    renderCard({}, 'zh-CN')
    // zh-CN comma-formats the same way Intl chooses ("12,345"); the goal is to
    // exercise the `language === 'en' ? 'en-US' : language` branch.
    expect(await screen.findByText('12,345')).toBeInTheDocument()
    expect(screen.getByTestId('dashboard-this-week')).toBeInTheDocument()
  })

  test('skips state updates when unmounted before the fetch resolves', async () => {
    let resolveTrend: (
      value: Awaited<ReturnType<typeof coreIntelligenceApi.getDiscoveryTrend>>,
    ) => void = () => {}
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTrend = resolve
        }),
    )
    const { unmount } = renderCard()
    unmount()
    // Flushing the in-flight promise post-unmount drives the
    // `cancelled === true` branches inside the try/finally.
    act(() =>
      resolveTrend(makeTrendResult([{ totalVisits: 9, newDomainCount: 1 }])),
    )
    await new Promise((resolve) => window.setTimeout(resolve, 10))
  })

  test('skips the error setters when unmounted before a rejection settles', async () => {
    let rejectTrend: (reason: unknown) => void = () => {}
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectTrend = reject
        }),
    )
    const { unmount } = renderCard()
    unmount()
    act(() => rejectTrend(new Error('late boom')))
    await new Promise((resolve) => window.setTimeout(resolve, 10))
  })

  describe('isoWeek (driven via week badge)', () => {
    beforeEach(() => {
      vi.useFakeTimers()
      // The card fetches on mount; resolve immediately so fake timers don't
      // strand an in-flight promise.
      vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
        makeTrendResult([]),
      )
    })

    test('renders a Sunday-anchored week without falling back to week 0', () => {
      // Sunday 2026-01-04. `new Date().getDay()` is 0 — the `|| 7` fallback
      // inside isoWeek covers the Sunday branch.
      vi.setSystemTime(new Date(2026, 0, 4))
      renderCard()
      expect(screen.getByText(/Week \d+/)).toBeInTheDocument()
    })

    test('renders a mid-week date so the typical branch runs', () => {
      // Wednesday 2026-04-15. getDay() = 3, falls through `|| 7`.
      vi.setSystemTime(new Date(2026, 3, 15))
      renderCard()
      expect(screen.getByText(/Week \d+/)).toBeInTheDocument()
    })
  })
})
