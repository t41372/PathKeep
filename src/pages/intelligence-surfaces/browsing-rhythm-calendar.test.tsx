/**
 * @file browsing-rhythm-calendar.test.tsx
 * @description Preserves the real-date browsing-rhythm calendar behavior and related hidden-tab prewarm contract.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Keep the calendar-style browsing-rhythm assertions identical to the original mega-suite.
 * - Guard the same-year range copy and post-paint search-activity prewarm behavior.
 *
 * ## Non-Responsibilities
 * - Does not own inline day-preview navigation tests.
 * - Does not redefine shared provider wiring or seeded archive fixtures.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence route harness in `test-helpers.tsx`.
 * - Uses the route-level `IntelligencePage` plus i18n translators from the shipped app.
 *
 * ## Performance Notes
 * - Keeps prewarm assertions explicit so split suites continue protecting first-paint behavior.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { createNamespaceTranslator } from '../../lib/i18n'
import { IntelligencePage } from '../intelligence'
import {
  createEmptyRuntimeSnapshot,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'

beforeEach(resetIntelligenceSurfaceHarness)

test('renders browsing rhythm as a real-date calendar and keeps secondary cards capped', async () => {
  const user = userEvent.setup()
  const { snapshot } = await seedArchiveState()
  const intelligenceT = createNamespaceTranslator('en', 'intelligence')

  vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
    createEmptyRuntimeSnapshot(),
  )
  vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockImplementation(
    (dateRange) =>
      Promise.resolve(
        wrapSection('digest-summary', {
          dateRange,
          totalVisits: {
            value: dateRange.start === dateRange.end ? 8 : 180,
            trend: 'flat' as const,
          },
          totalSearches: { value: 6, trend: 'flat' as const },
          newDomains: { value: 2, trend: 'flat' as const },
          deepReadPages: { value: 4, trend: 'flat' as const },
          refindPages: { value: 1, trend: 'flat' as const },
        }),
      ),
  )
  vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
    wrapSection('on-this-day', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getTopSites').mockImplementation((dateRange) =>
    Promise.resolve(
      wrapSection('top-sites', [
        {
          registrableDomain:
            dateRange.start === dateRange.end ? 'calendar.test' : 'example.com',
          displayName:
            dateRange.start === dateRange.end ? 'calendar.test' : 'example.com',
          domainCategory: 'docs',
          visitCount: dateRange.start === dateRange.end ? 8 : 24,
          uniqueDays: 1,
          averageDailyVisits: 8,
          uniqueUrls: 3,
        },
      ]),
    ),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
    wrapSection('engine-ranking', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
    wrapSection('search-concepts', [
      { term: 'sqlite', frequency: 4, engines: ['google'] },
    ]),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchQueries').mockResolvedValue(
    wrapSection('search-activity', {
      page: 0,
      pageSize: 20,
      total: 1,
      rows: [
        {
          visitId: 88,
          profileId: 'chrome:Default',
          browserKind: 'chrome',
          searchEngine: 'google',
          displayName: 'Google',
          rawQuery: 'sqlite wal checkpoint',
          normalizedQuery: 'sqlite wal checkpoint',
          searchedAt: '2026-01-28T09:30:00Z',
          searchedAtMs: 1_706_434_200_000,
          exactRepeatCount: 2,
          familyCount: 4,
          familyId: 'family-1',
          trailId: 'trail-1',
          trailInitialQuery: 'sqlite wal checkpoint',
          trailReformulationCount: 3,
        },
      ],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
    wrapSection('query-families', {
      page: 0,
      pageSize: 10,
      total: 0,
      families: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
    wrapSection('refind-pages', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
    wrapSection('activity-mix', {
      categories: [
        { domainCategory: 'docs', visitCount: 24, share: 0.6 },
        { domainCategory: 'search', visitCount: 16, share: 0.4 },
      ],
      changeVsPrevious: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockImplementation(
    (dateRange) =>
      Promise.resolve(
        wrapSection('browsing-rhythm', {
          cells:
            dateRange.start === dateRange.end
              ? [
                  { dow: 6, hour: 9, visitCount: 2 },
                  { dow: 6, hour: 10, visitCount: 6 },
                  { dow: 6, hour: 14, visitCount: 4 },
                ]
              : [],
          maxCount: dateRange.start === dateRange.end ? 6 : 0,
        }),
      ),
  )
  vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
    wrapSection('stable-sources', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
    wrapSection('search-effectiveness', {
      engineStats: [],
      topResolvingSources: [],
      hardestTopics: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
    wrapSection('friction-signals', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getReopenedInvestigations').mockResolvedValue(
    wrapSection('reopened-investigations', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
    (_dateRange, _profileId, granularity) =>
      Promise.resolve(
        wrapSection('discovery-trend', {
          availableYears: [2026],
          points:
            granularity === 'day'
              ? [
                  {
                    dateKey: '2026-01-03',
                    discoveryRate: 0.25,
                    newDomainCount: 1,
                    totalVisits: 4,
                  },
                  {
                    dateKey: '2026-01-17',
                    discoveryRate: 0.125,
                    newDomainCount: 1,
                    totalVisits: 8,
                  },
                  {
                    dateKey: '2026-01-30',
                    discoveryRate: 0.25,
                    newDomainCount: 2,
                    totalVisits: 8,
                  },
                ]
              : [
                  {
                    dateKey: '2026-W01',
                    discoveryRate: 0.14,
                    newDomainCount: 3,
                    totalVisits: 21,
                  },
                  {
                    dateKey: '2026-W04',
                    discoveryRate: 0.22,
                    newDomainCount: 5,
                    totalVisits: 23,
                  },
                ],
        }),
      ),
  )
  vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
    wrapSection('breadth-index', {
      breadthScore: 62,
      hhi: 0.32,
      concentrationDomainCount: 5,
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
    wrapSection('path-flows', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
    wrapSection('habit-patterns', [
      {
        registrableDomain: 'linux.do',
        displayName: 'linux.do',
        habitType: 'daily_habit',
        meanIntervalDays: 1.8,
        cv: 0.2,
        visitCount: 12,
        lastVisitedAt: '2026-01-30T08:00:00Z',
        isInterrupted: false,
      },
    ]),
  )
  vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
    wrapSection('interrupted-habits', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
    wrapSection('compare-sets', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
    wrapSection('multi-browser-diff', {
      profiles: [],
      sharedDomains: [],
      exclusiveDomains: [],
      categoryDistributions: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
    wrapSection('observed-interactions', []),
  )

  const { container } = renderSurface(<IntelligencePage />, {
    language: 'en',
    route: '/intelligence?range=custom&start=2026-01-01&end=2026-01-31',
    snapshot,
  })

  const firstDayButton = await screen.findByRole('button', {
    name: /2026-01-03 · 4 visits · 1 new sites/i,
  })
  expect(firstDayButton).toBeVisible()
  expect(firstDayButton).toHaveAttribute(
    'title',
    '2026-01-03 · 4 visits · 1 new sites',
  )
  expect(screen.getByTestId('browsing-rhythm-summary')).toHaveTextContent(
    '20 visits in January 2026',
  )
  expect(
    vi
      .mocked(coreIntelligenceApi.getDiscoveryTrend)
      .mock.calls.filter(([, , granularity]) => granularity === 'day'),
  ).toHaveLength(1)
  expect(
    screen.getByRole('button', {
      name: /2026-01-30 · 8 visits · 2 new sites/i,
    }),
  ).toBeVisible()
  expect(screen.getByText('2026 Week 4')).toBeVisible()

  const searchSection = screen.getByText('Search Activity').closest('section')
  const mixSection = screen.getByText('Activity Mix').closest('section')
  const sharedRow = searchSection?.parentElement
  expect(sharedRow).toHaveClass('intelligence-row--two-col')
  expect(sharedRow).toContainElement(mixSection)
  expect(
    searchSection?.querySelector('.intelligence-section__body'),
  ).not.toBeNull()
  expect(
    mixSection?.querySelector('.intelligence-section__body'),
  ).not.toBeNull()
  expect(
    screen
      .getByText('Browsing Rhythm')
      .closest('section')
      ?.querySelector('.intelligence-section__body--workbench'),
  ).not.toBeNull()
  expect(container.querySelector('.intelligence-secondary-grid')).not.toBeNull()
  expect(
    screen.queryByRole('heading', {
      name: intelligenceT('onThisDayTitle'),
    }),
  ).not.toBeInTheDocument()

  await user.click(screen.getByRole('tab', { name: 'Top Concepts' }))
  expect(
    await screen.findByText(
      /These bars rank the concepts that appeared most often/i,
    ),
  ).toBeVisible()
  await waitFor(() =>
    expect(
      container.querySelector('.search-concepts-chart__bars'),
    ).not.toBeNull(),
  )

  await user.click(screen.getByRole('tab', { name: 'Search Keywords' }))
  const searchSectionScope = within(searchSection as HTMLElement)
  expect(searchSectionScope.getByLabelText('Start date')).toHaveValue(
    '2026-01-01',
  )
  expect(searchSectionScope.getByLabelText('End date')).toHaveValue(
    '2026-01-31',
  )
  expect(
    searchSectionScope.getByRole('button', { name: 'Reset range' }),
  ).toBeVisible()
  expect(searchSectionScope.getByText('Page 1 of 1')).toBeVisible()
  expect(searchSectionScope.getByText('Showing 1 rows out of 1')).toBeVisible()
  expect(searchSectionScope.getByLabelText('Rows')).toHaveValue('20')
  expect(
    await searchSectionScope.findByRole('link', {
      name: 'Open query-family insights',
    }),
  ).toHaveAttribute(
    'href',
    '/intelligence/query-family/family-1?range=custom&start=2026-01-01&end=2026-01-31&profileId=chrome%3ADefault',
  )
  expect(
    searchSectionScope.getByRole('link', { name: 'Open trail insights' }),
  ).toHaveAttribute(
    'href',
    '/intelligence/trail/trail-1?range=custom&start=2026-01-01&end=2026-01-31&profileId=chrome%3ADefault',
  )
})

test('formats same-year browsing rhythm ranges without repeating the year', async () => {
  const { snapshot } = await seedArchiveState()

  vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
    createEmptyRuntimeSnapshot(),
  )
  vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
    wrapSection('digest-summary', {
      dateRange: { start: '2026-03-20', end: '2026-04-20' },
      totalVisits: { value: 180, trend: 'flat' as const },
      totalSearches: { value: 24, trend: 'flat' as const },
      newDomains: { value: 8, trend: 'flat' as const },
      deepReadPages: { value: 12, trend: 'flat' as const },
      refindPages: { value: 4, trend: 'flat' as const },
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
    wrapSection('top-sites', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
    wrapSection('engine-ranking', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
    wrapSection('search-concepts', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchQueries').mockResolvedValue(
    wrapSection('search-activity', {
      page: 0,
      pageSize: 20,
      total: 0,
      rows: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
    wrapSection('query-families', {
      page: 0,
      pageSize: 10,
      total: 0,
      families: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
    wrapSection('refind-pages', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
    wrapSection('activity-mix', {
      categories: [],
      changeVsPrevious: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
    wrapSection('discovery-trend', {
      availableYears: [2026],
      points: [
        {
          dateKey: '2026-03-20',
          discoveryRate: 0.25,
          newDomainCount: 1,
          totalVisits: 80,
        },
        {
          dateKey: '2026-04-20',
          discoveryRate: 0.2,
          newDomainCount: 2,
          totalVisits: 40,
        },
      ],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
    wrapSection('stable-sources', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
    wrapSection('search-effectiveness', {
      engineStats: [],
      topResolvingSources: [],
      hardestTopics: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
    wrapSection('friction-signals', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getReopenedInvestigations').mockResolvedValue(
    wrapSection('reopened-investigations', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
    wrapSection('breadth-index', {
      breadthScore: 62,
      hhi: 0.32,
      concentrationDomainCount: 5,
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
    wrapSection('path-flows', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
    wrapSection('habit-patterns', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
    wrapSection('interrupted-habits', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
    wrapSection('compare-sets', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
    wrapSection('multi-browser-diff', {
      profiles: [],
      sharedDomains: [],
      exclusiveDomains: [],
      categoryDistributions: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
    wrapSection('observed-interactions', []),
  )

  renderSurface(<IntelligencePage />, {
    language: 'zh-TW',
    route: '/intelligence?range=custom&start=2026-03-20&end=2026-04-20',
    snapshot,
  })

  expect(
    await screen.findByTestId('browsing-rhythm-summary'),
  ).toHaveTextContent('2026年 3月20日 至 4月20日，共瀏覽 120 次')
})

test('prewarms search-activity hidden tabs after first paint instead of waiting for a click', async () => {
  const { snapshot } = await seedArchiveState()

  vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
    createEmptyRuntimeSnapshot(),
  )
  vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
    wrapSection('digest-summary', {
      dateRange: { start: '2026-04-01', end: '2026-04-30' },
      totalVisits: { value: 180, trend: 'flat' as const },
      totalSearches: { value: 24, trend: 'flat' as const },
      newDomains: { value: 8, trend: 'flat' as const },
      deepReadPages: { value: 12, trend: 'flat' as const },
      refindPages: { value: 4, trend: 'flat' as const },
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
    wrapSection('top-sites', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
    wrapSection('engine-ranking', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
    wrapSection('search-concepts', []),
  )
  const queriesSpy = vi
    .spyOn(coreIntelligenceApi, 'getSearchQueries')
    .mockResolvedValue(
      wrapSection('search-activity', {
        page: 0,
        pageSize: 20,
        total: 0,
        rows: [],
      }),
    )
  const familiesSpy = vi
    .spyOn(coreIntelligenceApi, 'getQueryFamilies')
    .mockResolvedValue(
      wrapSection('query-families', {
        page: 0,
        pageSize: 10,
        total: 0,
        families: [],
      }),
    )
  vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
    wrapSection('refind-pages', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
    wrapSection('activity-mix', {
      categories: [],
      changeVsPrevious: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
    wrapSection('discovery-trend', { points: [], availableYears: [2026] }),
  )
  vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
    wrapSection('stable-sources', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
    wrapSection('search-effectiveness', {
      engineStats: [],
      topResolvingSources: [],
      hardestTopics: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
    wrapSection('friction-signals', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getReopenedInvestigations').mockResolvedValue(
    wrapSection('reopened-investigations', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
    wrapSection('breadth-index', {
      breadthScore: 0,
      hhi: 0,
      concentrationDomainCount: 0,
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
    wrapSection('path-flows', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
    wrapSection('habit-patterns', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
    wrapSection('interrupted-habits', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
    wrapSection('compare-sets', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
    wrapSection('multi-browser-diff', {
      profiles: [],
      sharedDomains: [],
      exclusiveDomains: [],
      categoryDistributions: [],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
    wrapSection('observed-interactions', []),
  )

  renderSurface(<IntelligencePage />, {
    route: '/intelligence',
    snapshot,
  })

  await screen.findByRole('heading', { name: 'Search Activity' })
  expect(screen.getByRole('tab', { name: 'Engine Ranking' })).toHaveAttribute(
    'aria-selected',
    'true',
  )
  expect(screen.getByRole('tab', { name: 'Search Keywords' })).toHaveAttribute(
    'aria-selected',
    'false',
  )
  await waitFor(() => expect(queriesSpy).toHaveBeenCalled(), { timeout: 3000 })
  await waitFor(() => expect(familiesSpy).toHaveBeenCalled(), { timeout: 3000 })
})
