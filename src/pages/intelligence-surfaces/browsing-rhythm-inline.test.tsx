/**
 * @file browsing-rhythm-inline.test.tsx
 * @description Protects the inline day-preview contract for the Intelligence browsing-rhythm surface.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the "preview inline first, navigate only on explicit CTA" behavior.
 * - Keep the day-detail drill-in assertions aligned with the original mega-suite.
 *
 * ## Non-Responsibilities
 * - Does not own the browsing-rhythm calendar layout suite.
 * - Does not redefine the shared route harness or archive seeding helpers.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence surface harness in `test-helpers.tsx`.
 * - Uses the route-level `IntelligencePage` and typed Core Intelligence section envelopes.
 *
 * ## Performance Notes
 * - Reuses the seeded archive helper so the split suite does not rebuild bespoke app state.
 */

import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router-dom'
import { beforeEach, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type { DateRange, DayInsights } from '../../lib/core-intelligence/types'
import { IntelligencePage } from '../intelligence'
import {
  createEmptyRuntimeSnapshot,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'

beforeEach(resetIntelligenceSurfaceHarness)

test('loads browsing-rhythm day preview inline and only navigates on explicit detail CTA', async () => {
  const user = userEvent.setup()
  const { snapshot } = await seedArchiveState()

  vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
    createEmptyRuntimeSnapshot(),
  )
  const getDigestSummarySpy = vi
    .spyOn(coreIntelligenceApi, 'getDigestSummary')
    .mockImplementation((dateRange) => {
      if (dateRange.start === dateRange.end) {
        return Promise.resolve(
          wrapSection('digest-summary', {
            dateRange,
            totalVisits: { value: 42, trend: 'flat' as const },
            totalSearches: { value: 8, trend: 'flat' as const },
            newDomains: { value: 2, trend: 'flat' as const },
            deepReadPages: { value: 5, trend: 'flat' as const },
            refindPages: { value: 1, trend: 'flat' as const },
          }),
        )
      }

      return Promise.resolve(
        wrapSection('digest-summary', {
          dateRange,
          totalVisits: { value: 240, trend: 'flat' as const },
          totalSearches: { value: 24, trend: 'flat' as const },
          newDomains: { value: 9, trend: 'flat' as const },
          deepReadPages: { value: 18, trend: 'flat' as const },
          refindPages: { value: 4, trend: 'flat' as const },
        }),
      )
    })
  const getDayInsightsSpy = vi
    .spyOn(coreIntelligenceApi, 'getDayInsights')
    .mockResolvedValue(
      wrapSection<DayInsights>('day-insights', {
        date: '2026-04-15',
        digestSummary: {
          dateRange: {
            start: '2026-04-15',
            end: '2026-04-15',
          } satisfies DateRange,
          totalVisits: { value: 42, trend: 'flat' as const },
          totalSearches: { value: 8, trend: 'flat' as const },
          newDomains: { value: 2, trend: 'flat' as const },
          deepReadPages: { value: 5, trend: 'flat' as const },
          refindPages: { value: 1, trend: 'flat' as const },
        },
        topSites: [
          {
            registrableDomain: 'sqlite.org',
            displayName: 'sqlite.org',
            domainCategory: 'docs',
            visitCount: 14,
            uniqueDays: 1,
            averageDailyVisits: 14,
            uniqueUrls: 4,
          },
        ],
        activityMix: {
          categories: [{ domainCategory: 'docs', visitCount: 42, share: 1 }],
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
          visitCount: hour === 10 ? 9 : 0,
        })),
        drilldown: {
          explorerDateRange: { start: '2026-04-15', end: '2026-04-15' },
        },
      }),
    )
  vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
    wrapSection('on-this-day', []),
  )
  const getTopSitesSpy = vi
    .spyOn(coreIntelligenceApi, 'getTopSites')
    .mockImplementation((dateRange) => {
      if (dateRange.start === dateRange.end) {
        return Promise.resolve(
          wrapSection('top-sites', [
            {
              registrableDomain: 'sqlite.org',
              displayName: 'sqlite.org',
              domainCategory: 'docs',
              visitCount: 14,
              uniqueDays: 1,
              averageDailyVisits: 14,
              uniqueUrls: 4,
            },
          ]),
        )
      }

      return Promise.resolve(
        wrapSection('top-sites', [
          {
            registrableDomain: 'example.com',
            displayName: 'example.com',
            domainCategory: 'docs',
            visitCount: 20,
            uniqueDays: 5,
            averageDailyVisits: 4,
            uniqueUrls: 6,
          },
        ]),
      )
    })
  vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
    wrapSection('engine-ranking', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
    wrapSection('search-concepts', [
      { term: 'sqlite', frequency: 4, engines: ['google'] },
    ]),
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
      categories: [{ domainCategory: 'docs', visitCount: 20, share: 1 }],
      changeVsPrevious: [],
    }),
  )
  const getBrowsingRhythmSpy = vi
    .spyOn(coreIntelligenceApi, 'getBrowsingRhythm')
    .mockImplementation((dateRange) =>
      Promise.resolve(
        wrapSection('browsing-rhythm', {
          cells:
            dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15'
              ? [{ dow: 3, hour: 10, visitCount: 9 }]
              : [{ dow: 4, hour: 8, visitCount: 4 }],
          maxCount:
            dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15'
              ? 9
              : 4,
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
          availableYears: [2026, 2025],
          points:
            granularity === 'day'
              ? [
                  {
                    dateKey: '2026-04-15',
                    discoveryRate: 0.18,
                    newDomainCount: 2,
                    totalVisits: 42,
                  },
                  {
                    dateKey: '2026-04-16',
                    discoveryRate: 0.1,
                    newDomainCount: 1,
                    totalVisits: 12,
                  },
                ]
              : [],
        }),
      ),
  )
  vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
    wrapSection('breadth-index', {
      breadthScore: 42,
      hhi: 0.42,
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

  renderSurface(
    <Routes>
      <Route path="/intelligence" element={<IntelligencePage />} />
      <Route
        path="/intelligence/day/:date"
        element={<div data-testid="day-insights-route-target" />}
      />
    </Routes>,
    {
      language: 'en',
      route: '/intelligence',
      snapshot,
    },
  )

  const rhythmSection = (await screen.findByText('Browsing Rhythm')).closest(
    'section',
  )
  expect(rhythmSection).not.toBeNull()
  expect(
    await within(rhythmSection!).findByTestId(
      'intelligence-section-meta-discovery-trend',
    ),
  ).toBeVisible()
  expect(getDayInsightsSpy).not.toHaveBeenCalled()

  await user.click(
    await screen.findByRole('button', {
      name: /2026-04-15 · 42 visits · 2 new sites/i,
    }),
  )

  const dayDetail = await screen.findByTestId('browsing-rhythm-day-detail')
  expect(dayDetail).toBeVisible()
  expect(getDayInsightsSpy).toHaveBeenCalledWith('2026-04-15', null)
  expect(within(dayDetail).getByTestId('rhythm-hour-strip')).toBeVisible()
  expect(
    within(dayDetail).getByTestId('rhythm-activity-proportion'),
  ).toBeVisible()
  expect(
    screen.queryByTestId('day-insights-route-target'),
  ).not.toBeInTheDocument()
  expect(within(dayDetail).getByText('sqlite.org')).toBeVisible()
  expect(within(dayDetail).getByText('Activity Mix')).toBeVisible()
  expect(screen.getByRole('link', { name: 'View details' })).toHaveAttribute(
    'href',
    '/intelligence/day/2026-04-15',
  )

  expect(
    getDigestSummarySpy.mock.calls.some(
      ([dateRange]) =>
        dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15',
    ),
  ).toBe(false)
  expect(
    getTopSitesSpy.mock.calls.some(
      ([dateRange]) =>
        dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15',
    ),
  ).toBe(false)
  expect(
    getBrowsingRhythmSpy.mock.calls.some(
      ([dateRange]) =>
        dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15',
    ),
  ).toBe(false)
})
