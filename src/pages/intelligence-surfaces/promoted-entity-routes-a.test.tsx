/**
 * @file promoted-entity-routes-a.test.tsx
 * @description Preserves the first half of route-first promoted Intelligence entity destinations.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Keep day-insight route promises intact.
 * - Keep query-family route promises intact.
 * - Keep refind-page route promises intact.
 * - Keep compare-set route promises intact.
 *
 * ## Non-Responsibilities
 * - Does not own grouped Explorer view assertions.
 * - Does not own session/trail follow-up entity routes from the second promoted route slice.
 * - Does not introduce new route helpers or alter shared harness behavior.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence surface harness in `test-helpers.tsx`.
 * - Uses route-first Intelligence pages as shipped integration surfaces.
 * - Talks to `core-intelligence/api` mocks and typed route payloads.
 *
 * ## Performance Notes
 * - Uses the seeded archive fixture once per test and keeps route assertions targeted.
 * - Reuses shared wrappers so the split suite does not duplicate setup-heavy scaffolding.
 */

import { screen, within } from '@testing-library/react'
import { Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type {
  CompareSetDetail,
  DayInsights,
  QueryFamilyDetail,
  RefindPageDetail,
} from '../../lib/core-intelligence/types'
import { createNamespaceTranslator } from '../../lib/i18n'
import {
  CompareSetInsightsRoutePage,
  DayInsightsRoutePage,
  QueryFamilyInsightsRoutePage,
  RefindPageInsightsRoutePage,
} from '../intelligence'
import {
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders day insights as a first-class route with exact-day explorer and domain links', async () => {
    const { snapshot } = await seedArchiveState()
    const dayInsightsSpy = vi
      .spyOn(coreIntelligenceApi, 'getDayInsights')
      .mockResolvedValue(
        wrapSection<DayInsights>(
          'day-insights',
          {
            date: '2026-04-18',
            digestSummary: {
              dateRange: { start: '2026-04-18', end: '2026-04-18' },
              totalVisits: { value: 8, trend: 'flat' },
              totalSearches: { value: 3, trend: 'flat' },
              newDomains: { value: 2, trend: 'flat' },
              deepReadPages: { value: 4, trend: 'flat' },
              refindPages: { value: 1, trend: 'flat' },
            },
            topSites: [
              {
                registrableDomain: 'sqlite.org',
                displayName: 'SQLite',
                domainCategory: 'docs',
                visitCount: 4,
                uniqueDays: 1,
                averageDailyVisits: 4,
                uniqueUrls: 2,
              },
            ],
            activityMix: {
              categories: [{ domainCategory: 'docs', visitCount: 8, share: 1 }],
              changeVsPrevious: [],
            },
            refindPages: [
              {
                canonicalUrl: 'https://sqlite.org/lang.html',
                url: 'https://sqlite.org/lang.html',
                title: 'SQLite docs',
                registrableDomain: 'sqlite.org',
                crossDayCount: 3,
                trailCount: 2,
                searchArrivalCount: 1,
                typedRevisitCount: 0,
                refindScore: 5,
                firstSeenAt: '2026-04-10T00:00:00Z',
                lastSeenAt: '2026-04-18T00:00:00Z',
              },
            ],
            queryFamilies: {
              families: [
                {
                  familyId: 'family-1',
                  anchorQuery: 'sqlite wal',
                  memberCount: 3,
                  searchEngine: 'google',
                  queries: ['sqlite wal', 'sqlite checkpoint'],
                  firstSeenAt: '2026-04-18T00:00:00Z',
                  lastSeenAt: '2026-04-18T01:00:00Z',
                },
              ],
              total: 1,
              page: 0,
              pageSize: 8,
            },
            hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
              hour,
              visitCount: hour === 10 ? 4 : 0,
            })),
            drilldown: {
              explorerDateRange: { start: '2026-04-18', end: '2026-04-18' },
            },
          },
          {
            moduleIds: [
              'daily-rollups',
              'search-trails',
              'refind-pages',
              'activity-mix',
            ],
            sourceTables: [
              'daily_summary_rollups',
              'domain_daily_rollups',
              'category_daily_rollups',
              'query_families',
              'refind_pages',
            ],
          },
        ),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/2026-04-18?profileId=chrome:Default',
        snapshot,
      },
    )

    expect(await screen.findByTestId('day-insights-page')).toBeVisible()
    expect(dayInsightsSpy).toHaveBeenCalledWith('2026-04-18', 'chrome:Default')
    expect(
      screen.getByRole('link', { name: 'Open exact-day evidence' }),
    ).toHaveAttribute(
      'href',
      '/explorer?profileId=chrome%3ADefault&start=2026-04-18&end=2026-04-18',
    )
    expect(screen.getByRole('link', { name: 'SQLite docs' })).toHaveAttribute(
      'href',
      '/intelligence/refind/https%3A%2F%2Fsqlite.org%2Flang.html?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
    )
    expect(
      screen.queryByRole('button', { name: /Show score factors/i }),
    ).not.toBeInTheDocument()
    const topSitesSection = screen
      .getByRole('heading', { name: 'Standout Sites' })
      .closest('section')
    if (!(topSitesSection instanceof HTMLElement)) {
      throw new Error('expected standout sites section')
    }
    expect(
      within(topSitesSection).getByRole('link', { name: /SQLite/i }),
    ).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
    )
    expect(screen.getByTestId('rhythm-hour-strip')).toBeVisible()
    expect(screen.getByTestId('rhythm-activity-proportion')).toBeVisible()
    expect(
      topSitesSection.closest('.intelligence-secondary-grid'),
    ).not.toBeNull()
  })

  test('keeps invalid and unavailable day-insight routes honest', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const dayInsightsSpy = vi.spyOn(coreIntelligenceApi, 'getDayInsights')

    const invalidRoute = renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/not-a-date?profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('dayInsightsInvalidDate')),
    ).toBeVisible()
    expect(dayInsightsSpy).not.toHaveBeenCalled()
    invalidRoute.unmount()

    dayInsightsSpy.mockRejectedValueOnce(new Error('day insight unavailable'))
    renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/2026-04-18?profileId=chrome:Default',
        snapshot,
      },
    )

    expect(await screen.findByText('day insight unavailable')).toBeVisible()
  })

  test('renders empty and fallback day-insight branches without losing route actions', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    vi.spyOn(coreIntelligenceApi, 'getDayInsights').mockResolvedValue(
      wrapSection<DayInsights>('day-insights', null as unknown as DayInsights),
    )

    const emptyView = renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/2026-04-18',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('dayInsightsEmpty')),
    ).toBeVisible()
    emptyView.unmount()

    vi.spyOn(coreIntelligenceApi, 'getDayInsights').mockResolvedValue(
      wrapSection<DayInsights>(
        'day-insights',
        dayInsightsFixture({
          activityMix: {
            categories: [],
            changeVsPrevious: [],
          },
          hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
            hour,
            visitCount: 0,
          })),
          queryFamilies: {
            families: [],
            total: 0,
            page: 0,
            pageSize: 8,
          },
          refindPages: [
            {
              canonicalUrl: 'https://fallback.test/page',
              url: 'https://fallback.test/page',
              title: null,
              registrableDomain: 'fallback.test',
              crossDayCount: 2,
              trailCount: 1,
              searchArrivalCount: 0,
              typedRevisitCount: 1,
              refindScore: 3,
              firstSeenAt: '2026-04-17T00:00:00Z',
              lastSeenAt: '2026-04-18T00:00:00Z',
            },
          ],
          topSites: [
            {
              registrableDomain: 'fallback.test',
              displayName: null,
              domainCategory: 'docs',
              visitCount: 0,
              uniqueDays: 1,
              averageDailyVisits: 0,
              uniqueUrls: 1,
            },
          ],
        }),
      ),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/2026-04-18',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('rhythmDayNoHourlyData')),
    ).toBeVisible()
    expect(screen.getByText(intelligenceT('activityMixEmpty'))).toBeVisible()
    expect(
      screen.getByText(intelligenceT('queryFamiliesPlaceholder')),
    ).toBeVisible()
    expect(
      screen
        .getAllByRole('link', { name: /fallback\.test/i })
        .some(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/domain/fallback.test?range=custom&start=2026-04-18&end=2026-04-18',
        ),
    ).toBe(true)
    expect(
      screen.getByRole('link', { name: 'https://fallback.test/page' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/refind/https%3A%2F%2Ffallback.test%2Fpage?range=custom&start=2026-04-18&end=2026-04-18',
    )
  })

  test('shows focused compare-set context only when the day belongs to it', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    vi.spyOn(coreIntelligenceApi, 'getDayInsights').mockResolvedValue(
      wrapSection<DayInsights>('day-insights', dayInsightsFixture()),
    )
    const compareSetSpy = vi.spyOn(coreIntelligenceApi, 'getCompareSetDetail')

    compareSetSpy.mockResolvedValueOnce(
      wrapSection<CompareSetDetail>(
        'compare-set-detail',
        compareSetDetailFixture(['2026-04-17']),
      ),
    )
    const notMatched = renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/day/2026-04-18?focusType=compare-set&focusId=compare-1',
        snapshot,
      },
    )

    expect(await screen.findByTestId('day-insights-page')).toBeVisible()
    expect(
      screen.queryByText(intelligenceT('compareSetFocusTitle')),
    ).not.toBeInTheDocument()
    notMatched.unmount()

    compareSetSpy.mockResolvedValueOnce(
      wrapSection<CompareSetDetail>(
        'compare-set-detail',
        compareSetDetailFixture(['2026-04-18']),
      ),
    )
    renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/day/2026-04-18?profileId=chrome:Default&focusType=compare-set&focusId=compare-1',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('compareSetFocusTitle')),
    ).toBeVisible()
    expect(compareSetSpy).toHaveBeenLastCalledWith(
      'compare-1',
      { start: '2026-04-18', end: '2026-04-18' },
      'chrome:Default',
    )
  })

  test('renders query-family insights as a first-class route with related trail links', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceTw = createNamespaceTranslator('zh-TW', 'intelligence')
    const detailSpy = vi
      .spyOn(coreIntelligenceApi, 'getQueryFamilyDetail')
      .mockResolvedValue(
        wrapSection<QueryFamilyDetail>(
          'query-family-detail',
          {
            family: {
              familyId: 'family-1',
              anchorQuery: 'sqlite wal',
              memberCount: 3,
              searchEngine: 'google',
              queries: ['sqlite wal', 'sqlite checkpoint'],
              firstSeenAt: '2026-04-18T00:00:00Z',
              lastSeenAt: '2026-04-18T01:00:00Z',
            },
            relatedTrails: [
              {
                trailId: 'trail-1',
                sessionId: 'session-1',
                initialQuery: 'sqlite wal',
                searchEngine: 'google',
                reformulationCount: 1,
                visitCount: 2,
                landingUrl: 'https://sqlite.org/wal.html',
                landingDomain: 'sqlite.org',
                firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
                lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
                maxDepth: 2,
                queries: ['sqlite wal', 'sqlite checkpoint'],
              },
            ],
          },
          {
            moduleIds: ['search-trails'],
            sourceTables: ['query_families', 'search_trails', 'search_events'],
          },
        ),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/query-family/:familyId"
          element={<QueryFamilyInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/query-family/family-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome:Default',
        language: 'zh-TW',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite wal/i }),
    ).toBeVisible()
    expect(
      screen.getByText(intelligenceTw('queryFamilyRouteTitle')),
    ).toBeVisible()
    expect(
      screen.getByText(intelligenceTw('queryFamilyQueriesTitle')),
    ).toBeVisible()
    expect(
      screen.getByText(intelligenceTw('searchQueriesEngineFilter')),
    ).toBeVisible()
    expect(
      screen.queryByText('INTELLIGENCE.QUERYFAMILYROUTETITLE'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('INTELLIGENCE.QUERYFAMILYQUERIESTITLE'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('intelligence.searchQueriesEngineFilter'),
    ).not.toBeInTheDocument()
    expect(screen.queryByText('2026-04-18T00:00:00Z')).not.toBeInTheDocument()
    expect(detailSpy).toHaveBeenCalledWith(
      'family-1',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )
    expect(
      screen.getByRole('link', {
        name: intelligenceTw('entityOpenExplorer'),
      }),
    ).toHaveAttribute(
      'href',
      '/explorer?profileId=chrome%3ADefault&start=2026-04-01&end=2026-04-30&q=sqlite+wal',
    )
    expect(
      screen
        .getAllByRole('link', { name: /sqlite wal/i })
        .some(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
        ),
    ).toBe(true)
  })

  test('keeps query-family route placeholders, empty trails, and date fallbacks honest', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const detailSpy = vi.spyOn(coreIntelligenceApi, 'getQueryFamilyDetail')

    const placeholder = renderSurface(
      <Routes>
        <Route
          path="/intelligence/query-family"
          element={<QueryFamilyInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/query-family',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('queryFamiliesPlaceholder')),
    ).toBeVisible()
    expect(detailSpy).not.toHaveBeenCalled()
    placeholder.unmount()

    detailSpy.mockRejectedValueOnce(new Error('query family unavailable'))
    const errorView = renderSurface(
      <Routes>
        <Route
          path="/intelligence/query-family/:familyId"
          element={<QueryFamilyInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/query-family/family-error',
        snapshot,
      },
    )

    expect(await screen.findByText('query family unavailable')).toBeVisible()
    errorView.unmount()

    detailSpy.mockResolvedValueOnce(
      wrapSection<QueryFamilyDetail>(
        'query-family-detail',
        null as unknown as QueryFamilyDetail,
      ),
    )
    const emptyDetail = renderSurface(
      <Routes>
        <Route
          path="/intelligence/query-family/:familyId"
          element={<QueryFamilyInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/query-family/family-empty-detail',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('queryFamiliesPlaceholder')),
    ).toBeVisible()
    emptyDetail.unmount()

    detailSpy.mockResolvedValueOnce(
      wrapSection<QueryFamilyDetail>('query-family-detail', {
        family: {
          familyId: 'family-empty',
          anchorQuery: 'broken timestamp query',
          memberCount: 1,
          searchEngine: 'google',
          queries: ['broken timestamp query'],
          firstSeenAt: 'not-a-date',
          lastSeenAt: 'still-not-a-date',
        },
        relatedTrails: [],
      }),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/query-family/:familyId"
          element={<QueryFamilyInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/query-family/family-empty?range=custom&start=2026-04-01&end=2026-04-30',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', {
        name: /broken timestamp query/i,
      }),
    ).toBeVisible()
    expect(screen.getByText('not-a-date - still-not-a-date')).toBeVisible()
    expect(screen.getByText(intelligenceT('trailGroupEmpty'))).toBeVisible()
  })

  test('renders refind-page insights with day and trail drilldowns', async () => {
    const { snapshot } = await seedArchiveState()
    const detailSpy = vi
      .spyOn(coreIntelligenceApi, 'getRefindPageDetail')
      .mockResolvedValue(
        wrapSection<RefindPageDetail>(
          'refind-page-detail',
          {
            page: {
              canonicalUrl: 'https://sqlite.org/lang.html',
              url: 'https://sqlite.org/lang.html',
              title: 'SQLite docs',
              registrableDomain: 'sqlite.org',
              crossDayCount: 3,
              trailCount: 2,
              searchArrivalCount: 1,
              typedRevisitCount: 0,
              refindScore: 5,
              firstSeenAt: '2026-04-10T00:00:00Z',
              lastSeenAt: '2026-04-18T00:00:00Z',
            },
            explanation: {
              canonicalUrl: 'https://sqlite.org/lang.html',
              refindScore: 5,
              factors: [
                {
                  signal: 'cross_day_count',
                  rawValue: 3,
                  weight: 3,
                  contribution: 9,
                },
              ],
              visitIds: [101, 102],
            },
            recentDays: ['2026-04-18', '2026-04-12'],
            relatedTrails: [
              {
                trailId: 'trail-1',
                sessionId: 'session-1',
                initialQuery: 'sqlite wal',
                searchEngine: 'google',
                reformulationCount: 1,
                visitCount: 2,
                landingUrl: 'https://sqlite.org/lang.html',
                landingDomain: 'sqlite.org',
                firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
                lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
                maxDepth: 2,
                queries: ['sqlite wal', 'sqlite checkpoint'],
              },
            ],
          },
          {
            moduleIds: ['refind-pages', 'search-trails'],
            sourceTables: [
              'refind_pages',
              'visit_derived_facts',
              'search_trails',
            ],
          },
        ),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/refind/:canonicalUrl"
          element={<RefindPageInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/refind/https%3A%2F%2Fsqlite.org%2Flang.html?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: 'SQLite docs' }),
    ).toBeVisible()
    expect(detailSpy).toHaveBeenCalledWith(
      'https://sqlite.org/lang.html',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )
    expect(screen.getByRole('link', { name: '2026-04-18' })).toHaveAttribute(
      'href',
      '/intelligence/day/2026-04-18?profileId=chrome%3ADefault',
    )
    expect(screen.getByText('cross_day_count')).toBeVisible()
    expect(screen.getByText('3 ×3')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open domain insights' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: /sqlite wal/i })).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
    )
  })

  test('keeps refind route placeholders and empty optional detail lists honest', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const detailSpy = vi.spyOn(coreIntelligenceApi, 'getRefindPageDetail')

    const placeholder = renderSurface(
      <Routes>
        <Route
          path="/intelligence/refind"
          element={<RefindPageInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/refind',
        snapshot,
      },
    )

    expect(await screen.findByText(intelligenceT('refindEmpty'))).toBeVisible()
    expect(detailSpy).not.toHaveBeenCalled()
    placeholder.unmount()

    detailSpy.mockRejectedValueOnce(new Error('refind unavailable'))
    const errorView = renderSurface(
      <Routes>
        <Route
          path="/intelligence/refind/:canonicalUrl"
          element={<RefindPageInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/refind/https%3A%2F%2Fsqlite.org%2Ferror.html',
        snapshot,
      },
    )

    expect(await screen.findByText('refind unavailable')).toBeVisible()
    errorView.unmount()

    detailSpy.mockResolvedValueOnce(
      wrapSection<RefindPageDetail>(
        'refind-page-detail',
        null as unknown as RefindPageDetail,
      ),
    )
    const emptyDetail = renderSurface(
      <Routes>
        <Route
          path="/intelligence/refind/:canonicalUrl"
          element={<RefindPageInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/refind/https%3A%2F%2Fsqlite.org%2Fempty-detail.html',
        snapshot,
      },
    )

    expect(await screen.findByText(intelligenceT('refindEmpty'))).toBeVisible()
    emptyDetail.unmount()

    detailSpy.mockResolvedValueOnce(
      wrapSection<RefindPageDetail>('refind-page-detail', {
        page: {
          canonicalUrl: 'https://sqlite.org/empty.html',
          url: 'https://sqlite.org/empty.html',
          title: null,
          registrableDomain: 'sqlite.org',
          crossDayCount: 0,
          trailCount: 0,
          searchArrivalCount: 0,
          typedRevisitCount: 0,
          refindScore: 0,
          firstSeenAt: '2026-04-10T00:00:00Z',
          lastSeenAt: '2026-04-18T00:00:00Z',
        },
        explanation: {
          canonicalUrl: 'https://sqlite.org/empty.html',
          refindScore: 0,
          factors: [
            {
              signal: 'empty',
              rawValue: Number.POSITIVE_INFINITY,
              weight: Number.NaN,
              contribution: Number.NEGATIVE_INFINITY,
            },
          ],
          visitIds: [],
        },
        recentDays: [],
        relatedTrails: [],
      }),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/refind/:canonicalUrl"
          element={<RefindPageInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/refind/https%3A%2F%2Fsqlite.org%2Fempty.html?range=custom&start=2026-04-01&end=2026-04-30',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', {
        name: 'https://sqlite.org/empty.html',
      }),
    ).toBeVisible()
    expect(screen.getByText(intelligenceT('dayInsightsEmpty'))).toBeVisible()
    expect(screen.getByText(intelligenceT('trailGroupEmpty'))).toBeVisible()
    expect(screen.getByText('empty')).toBeVisible()
    expect(screen.getByText('0 ×0')).toBeVisible()
  })

  test('renders compare-set insights as a first-class route with focused trail and day links', async () => {
    const { snapshot } = await seedArchiveState()
    const detailSpy = vi
      .spyOn(coreIntelligenceApi, 'getCompareSetDetail')
      .mockResolvedValue(
        wrapSection('compare-set-detail', {
          compareSet: {
            compareSetId: 'compare:trail-1:docs_page',
            trailId: 'trail-1',
            searchQuery: 'sqlite wal',
            pageCategory: 'docs_page',
            pages: [
              {
                canonicalUrl: 'https://sqlite.org/wal.html',
                url: 'https://sqlite.org/wal.html',
                title: 'WAL mode',
                registrableDomain: 'sqlite.org',
                visitCount: 2,
                isLanding: true,
              },
              {
                canonicalUrl: 'https://sqlite.org/checkpoint.html',
                url: 'https://sqlite.org/checkpoint.html',
                title: 'Checkpoint',
                registrableDomain: 'sqlite.org',
                visitCount: 2,
                isLanding: false,
              },
            ],
          },
          trail: {
            trailId: 'trail-1',
            sessionId: 'session-1',
            initialQuery: 'sqlite wal',
            searchEngine: 'google',
            reformulationCount: 2,
            visitCount: 4,
            landingUrl: 'https://sqlite.org/wal.html',
            landingDomain: 'sqlite.org',
            firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
            lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
            maxDepth: 2,
            queries: ['sqlite wal', 'sqlite checkpoint'],
          },
          session: {
            sessionId: 'session-1',
            firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
            lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
            visitCount: 5,
            searchCount: 2,
            domainCount: 1,
            isDeepDive: false,
            autoTitle: 'SQLite compare',
          },
          recentDays: ['2026-04-18', '2026-04-12'],
        }),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/compare-set/:compareSetId"
          element={<CompareSetInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/compare-set/compare%3Atrail-1%3Adocs_page?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite wal/i }),
    ).toBeVisible()
    expect(detailSpy).toHaveBeenCalledWith(
      'compare:trail-1:docs_page',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )
    expect(
      screen.getByRole('link', { name: 'Open trail insights' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
    )
    expect(
      screen
        .getAllByRole('link', { name: 'sqlite.org' })
        .every(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/domain/sqlite.org?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        ),
    ).toBe(true)
    expect(
      screen
        .getAllByRole('link', { name: '2026-04-18' })
        .some(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/day/2026-04-18?profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        ),
    ).toBe(true)
  })

  test('keeps compare-set route placeholders and absent session context honest', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const detailSpy = vi.spyOn(coreIntelligenceApi, 'getCompareSetDetail')

    const placeholder = renderSurface(
      <Routes>
        <Route
          path="/intelligence/compare-set"
          element={<CompareSetInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/compare-set',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('compareSetsEmpty')),
    ).toBeVisible()
    expect(detailSpy).not.toHaveBeenCalled()
    placeholder.unmount()

    detailSpy.mockRejectedValueOnce(new Error('compare-set unavailable'))
    const errorView = renderSurface(
      <Routes>
        <Route
          path="/intelligence/compare-set/:compareSetId"
          element={<CompareSetInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/compare-set/compare-error',
        snapshot,
      },
    )

    expect(await screen.findByText('compare-set unavailable')).toBeVisible()
    errorView.unmount()

    detailSpy.mockResolvedValueOnce(
      wrapSection('compare-set-detail', null as unknown as CompareSetDetail),
    )
    const emptyDetail = renderSurface(
      <Routes>
        <Route
          path="/intelligence/compare-set/:compareSetId"
          element={<CompareSetInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/compare-set/compare-empty-detail',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('compareSetsEmpty')),
    ).toBeVisible()
    emptyDetail.unmount()

    detailSpy.mockResolvedValueOnce(
      wrapSection('compare-set-detail', {
        compareSet: {
          compareSetId: 'compare:trail-2:docs_page',
          trailId: 'trail-2',
          searchQuery: 'sqlite vacuum',
          pageCategory: 'docs_page',
          pages: [
            {
              canonicalUrl: 'https://sqlite.org/vacuum.html',
              url: 'https://sqlite.org/vacuum.html',
              title: null,
              registrableDomain: 'sqlite.org',
              visitCount: 0,
              isLanding: false,
            },
          ],
        },
        trail: {
          trailId: 'trail-2',
          sessionId: null,
          initialQuery: 'sqlite vacuum',
          searchEngine: 'google',
          reformulationCount: 0,
          visitCount: 1,
          landingUrl: 'https://sqlite.org/vacuum.html',
          landingDomain: 'sqlite.org',
          firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
          lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
          maxDepth: 1,
          queries: ['sqlite vacuum'],
        },
        session: null,
        recentDays: [],
      }),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/compare-set/:compareSetId"
          element={<CompareSetInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/compare-set/compare%3Atrail-2%3Adocs_page?range=custom&start=2026-04-01&end=2026-04-30',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite vacuum/i }),
    ).toBeVisible()
    expect(
      screen.queryByRole('link', {
        name: intelligenceT('compareSetRouteOpenSession'),
      }),
    ).not.toBeInTheDocument()
    expect(screen.getByText(intelligenceT('dayInsightsEmpty'))).toBeVisible()
  })
})

function dayInsightsFixture(overrides: Partial<DayInsights> = {}): DayInsights {
  return {
    date: '2026-04-18',
    digestSummary: {
      dateRange: { start: '2026-04-18', end: '2026-04-18' },
      totalVisits: { value: 8, trend: 'flat' },
      totalSearches: { value: 3, trend: 'flat' },
      newDomains: { value: 2, trend: 'flat' },
      deepReadPages: { value: 4, trend: 'flat' },
      refindPages: { value: 1, trend: 'flat' },
    },
    topSites: [
      {
        registrableDomain: 'sqlite.org',
        displayName: 'SQLite',
        domainCategory: 'docs',
        visitCount: 4,
        uniqueDays: 1,
        averageDailyVisits: 4,
        uniqueUrls: 2,
      },
    ],
    activityMix: {
      categories: [{ domainCategory: 'docs', visitCount: 8, share: 1 }],
      changeVsPrevious: [],
    },
    refindPages: [],
    queryFamilies: {
      families: [],
      total: 0,
      page: 0,
      pageSize: 8,
    },
    hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
      hour,
      visitCount: hour === 10 ? 4 : 0,
    })),
    drilldown: {
      explorerDateRange: { start: '2026-04-18', end: '2026-04-18' },
    },
    ...overrides,
  }
}

function compareSetDetailFixture(recentDays: string[]): CompareSetDetail {
  return {
    compareSet: {
      compareSetId: 'compare-1',
      trailId: 'trail-1',
      searchQuery: 'sqlite wal',
      pageCategory: 'docs',
      pages: [
        {
          canonicalUrl: 'https://sqlite.org/lang.html',
          url: 'https://sqlite.org/lang.html',
          title: 'SQLite docs',
          registrableDomain: 'sqlite.org',
          visitCount: 4,
          isLanding: true,
        },
      ],
    },
    recentDays,
    session: null,
    trail: {
      trailId: 'trail-1',
      sessionId: null,
      initialQuery: 'sqlite wal',
      searchEngine: 'google',
      reformulationCount: 0,
      visitCount: 1,
      landingUrl: 'https://sqlite.org/lang.html',
      landingDomain: 'sqlite.org',
      firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
      lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
      maxDepth: 1,
      queries: ['sqlite wal'],
    },
  }
}
