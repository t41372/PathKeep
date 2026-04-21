/**
 * @file health-and-storage.test.tsx
 * @description Protects the Intelligence health-tail ordering and storage analytics copy promises.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the grouped storage analytics assertions in the health tail.
 * - Keep the growth-signal ordering behavior and search-effectiveness plain-language copy unchanged.
 *
 * ## Non-Responsibilities
 * - Does not own browsing-rhythm interaction tests.
 * - Does not redefine the shared test harness or local-host fixtures.
 *
 * ## Dependencies
 * - Depends on the shared surface harness in `test-helpers.tsx`.
 * - Uses the shipped `intelligenceText` copy helper so locale fallbacks match production.
 *
 * ## Performance Notes
 * - Reuses seeded archive/dashboard snapshots to keep the split suite deterministic and bounded.
 */

import { screen, within } from '@testing-library/react'
import { beforeEach, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { createNamespaceTranslator } from '../../lib/i18n'
import { IntelligencePage } from '../intelligence'
import { intelligenceText } from '../intelligence/copy'
import {
  createEmptyRuntimeSnapshot,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'

beforeEach(resetIntelligenceSurfaceHarness)

test('renders grouped storage analytics in the intelligence health tail', async () => {
  const { snapshot, dashboard } = await seedArchiveState()
  const intelligenceT = createNamespaceTranslator('en', 'intelligence')
  const commonT = createNamespaceTranslator('en', 'common')

  vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
    createEmptyRuntimeSnapshot(),
  )
  vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
    wrapSection('digest-summary', {
      dateRange: { start: '2026-01-01', end: '2026-01-31' },
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
  vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockResolvedValue(
    wrapSection('browsing-rhythm', {
      cells: [],
      maxCount: 0,
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
  vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
    wrapSection('discovery-trend', { points: [], availableYears: [] }),
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
    dashboard,
    route: '/intelligence?range=custom&start=2026-01-01&end=2026-01-31',
    snapshot,
  })

  const storageHeading = await screen.findByRole('heading', {
    name: intelligenceText('en', intelligenceT, 'storageAnalytics'),
  })
  const storageSection = storageHeading.closest('section')
  if (!(storageSection instanceof HTMLElement)) {
    throw new Error('expected storage analytics section')
  }

  expect(storageHeading).toBeVisible()
  expect(
    within(storageSection).getAllByText(commonT('coreHistory')).length,
  ).toBeGreaterThan(0)
  expect(
    within(storageSection).getAllByText(commonT('otherData')).length,
  ).toBeGreaterThan(0)
  expect(
    within(storageSection).getByText(commonT('canonicalArchive')),
  ).toBeVisible()
  expect(
    within(storageSection).getByText(commonT('auditArtifacts')),
  ).toBeVisible()
})

test('moves an empty growth signal behind populated secondary cards', async () => {
  const { snapshot, dashboard } = await seedArchiveState()

  vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
    createEmptyRuntimeSnapshot(),
  )
  vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
    wrapSection('digest-summary', {
      dateRange: { start: '2026-01-01', end: '2026-01-31' },
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
    wrapSection('discovery-trend', { points: [], availableYears: [] }),
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
    dashboard: {
      ...dashboard,
      recentRuns: dashboard.recentRuns.map((run) => ({
        ...run,
        status: 'failed' as const,
      })),
    },
    route: '/intelligence?range=custom&start=2026-01-01&end=2026-01-31',
    snapshot,
  })

  const storageHeading = await screen.findByRole('heading', {
    name: 'Storage',
  })
  const breadthHeading = await screen.findByRole('heading', {
    name: 'Breadth Index',
  })
  const growthHeading = await screen.findByRole('heading', {
    name: 'Recent growth',
  })

  expect(
    storageHeading.compareDocumentPosition(growthHeading) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy()
  expect(
    breadthHeading.compareDocumentPosition(growthHeading) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy()
})

test('renders search effectiveness as plain-language summaries', async () => {
  const { snapshot } = await seedArchiveState()
  const intelligenceT = createNamespaceTranslator('en', 'intelligence')

  vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
    wrapSection('digest-summary', {
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      totalVisits: { value: 10, deltaPct: 0, trend: 'up' },
      totalSearches: { value: 4, deltaPct: 0, trend: 'up' },
      newDomains: { value: 3, deltaPct: 0, trend: 'up' },
      deepReadPages: { value: 2, deltaPct: 0, trend: 'up' },
      refindPages: { value: 1, deltaPct: 0, trend: 'up' },
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
    wrapSection('on-this-day', []),
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
  vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
    wrapSection('query-families', {
      page: 0,
      pageSize: 20,
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
  vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockResolvedValue(
    wrapSection('browsing-rhythm', {
      cells: [],
      maxCount: 0,
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
    wrapSection('stable-sources', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
    wrapSection('search-effectiveness', {
      engineStats: [
        {
          searchEngine: 'google',
          displayName: 'Google',
          avgReformulations: 1.2,
          totalTrails: 18,
          avgDepth: 2.4,
        },
      ],
      topResolvingSources: [
        {
          registrableDomain: 'developer.mozilla.org',
          displayName: 'MDN',
          sourceRole: 'landing',
          trailCount: 6,
          stableLandingCount: 6,
          effectivenessScore: 0.9,
        },
      ],
      hardestTopics: [
        {
          familyId: 'family-hardest-1',
          queryFamily: 'sqlite wal checkpoint',
          reformulationCount: 3,
          reSearchLagDays: 2.5,
        },
      ],
    }),
  )
  vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
    wrapSection('friction-signals', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getReopenedInvestigations').mockResolvedValue(
    wrapSection('reopened-investigations', []),
  )
  vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
    wrapSection('discovery-trend', { points: [], availableYears: [] }),
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

  renderSurface(<IntelligencePage />, {
    language: 'en',
    route: '/intelligence',
    snapshot,
  })

  expect(
    await screen.findByRole('heading', {
      name: intelligenceT('searchEffectivenessTitle'),
    }),
  ).toBeVisible()
  expect(
    await screen.findByText(
      'Each trail was rewritten about 1.2 times on average.',
    ),
  ).toBeVisible()
  expect(
    await screen.findByText('People usually stopped around depth 2.4.'),
  ).toBeVisible()
  expect(
    await screen.findByText('This window produced 18 search trails.'),
  ).toBeVisible()
  expect(screen.getByText('MDN')).toBeVisible()
  expect(screen.getByText('"sqlite wal checkpoint"')).toBeVisible()
})
