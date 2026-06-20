import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceSecondaryOverview,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  CoreIntelligenceSectionWindow,
  DateRange,
} from './types'

const { callMock } = vi.hoisted(() => ({
  callMock: vi.fn(),
}))

vi.mock('../backend-client/shared', () => ({
  call: callMock,
}))

const april: DateRange = { start: '2026-04-01', end: '2026-04-30' }

function testMeta(
  sectionId: string,
  window: CoreIntelligenceSectionWindow = {
    kind: 'date-range',
    dateRange: april,
  },
): CoreIntelligenceSectionMeta {
  return {
    sectionId,
    generatedAt: '2026-04-25T12:00:00Z',
    window,
    moduleIds: ['test-module'],
    sourceTables: ['test_table'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function testSection<T>(
  sectionId: string,
  data: T,
): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: testMeta(sectionId),
  }
}

describe('core intelligence overview cache helpers', () => {
  beforeEach(async () => {
    callMock.mockReset()
    callMock.mockResolvedValue({})
    const { clearIntelligenceOverviewCache } = await import('./api')
    clearIntelligenceOverviewCache()
  })

  test('fetches observed interactions over IPC when no secondary overview is cached', async () => {
    const { getObservedInteractions } = await import('./api')
    callMock.mockResolvedValueOnce([{ id: 'oi-1' }])

    await expect(
      getObservedInteractions(april, 'chrome:Default'),
    ).resolves.toMatchObject({
      data: [{ id: 'oi-1' }],
      meta: { sectionId: 'observed-interactions' },
    })
    expect(callMock).toHaveBeenCalledWith('get_observed_interactions', {
      request: { dateRange: april, profileId: 'chrome:Default' },
    })
  })

  test('reuses cached primary overview sections across peek and read helpers', async () => {
    const {
      getActivityMix,
      getHabitPatterns,
      getInterruptedHabits,
      getOnThisDay,
      getQueryFamilies,
      getRefindPages,
      getSearchEngineRanking,
      getTopSearchConcepts,
      getTopSites,
      peekActivityMix,
      peekDiscoveryTrend,
      peekHabitPatterns,
      peekInterruptedHabits,
      peekQueryFamilies,
      peekRefindPages,
      peekSearchEngineRanking,
      peekTopSearchConcepts,
      peekTopSites,
    } = await import('./api/overview')
    const { writeOverviewCache } = await import('./api/shared')
    const onThisDay = testSection('on-this-day', [{ year: 2025 }])
    const topSites = testSection('top-sites', [
      { domain: 'example.com' },
      { domain: 'docs.example.com' },
    ])
    const searchEngineRanking = testSection('search-activity', [
      { engine: 'google' },
    ])
    const topSearchConcepts = testSection('search-activity', [
      { term: 'tauri' },
      { term: 'sqlite' },
    ])
    const queryFamilies = testSection('search-activity', {
      page: 0,
      pageSize: 10,
      total: 2,
      families: [{ id: 'family-a' }, { id: 'family-b' }],
    })
    const refindPages = testSection('refind-pages', [
      { canonicalUrl: 'https://example.com/one' },
      { canonicalUrl: 'https://example.com/two' },
    ])
    const habitPatterns = testSection('habits', [{ id: 'habit-a' }])
    const interruptedHabits = testSection('habits', [{ id: 'interrupted-a' }])
    const activityMix = testSection('activity-mix', { categories: [] })
    const discoveryTrendDay = testSection('browsing-rhythm', { points: [] })

    writeOverviewCache(april, 'chrome:Default', () => ({
      primary: {
        digestSummary: testSection('digest-summary', {}),
        onThisDay,
        topSites,
        refindPages,
        searchEngineRanking,
        topSearchConcepts,
        queryFamilies,
        activityMix,
        discoveryTrendDay,
        habitPatterns,
        interruptedHabits,
        timings: [],
        totalDurationMs: 0,
      } as unknown as CoreIntelligencePrimaryOverview,
    }))

    expect(
      peekTopSites(april, 'chrome:Default', 'visit_count', 1)?.data,
    ).toEqual([{ domain: 'example.com' }])
    expect(peekTopSites(april, 'chrome:Default')?.data).toEqual([
      { domain: 'example.com' },
      { domain: 'docs.example.com' },
    ])
    expect(peekSearchEngineRanking(april, 'chrome:Default')).toBe(
      searchEngineRanking,
    )
    expect(peekTopSearchConcepts(april, 'chrome:Default', 1)?.data).toEqual([
      { term: 'tauri' },
    ])
    expect(peekTopSearchConcepts(april, 'chrome:Default')?.data).toEqual([
      { term: 'tauri' },
      { term: 'sqlite' },
    ])
    expect(
      peekQueryFamilies(april, 'chrome:Default', {
        page: 0,
        pageSize: 1,
      })?.data.families,
    ).toEqual([{ id: 'family-a' }])
    expect(peekQueryFamilies(april, 'chrome:Default')?.data).toMatchObject({
      page: 0,
      pageSize: 10,
      families: [{ id: 'family-a' }, { id: 'family-b' }],
    })
    expect(peekRefindPages(april, 'chrome:Default', 1)?.data).toEqual([
      { canonicalUrl: 'https://example.com/one' },
    ])
    expect(peekRefindPages(april, 'chrome:Default')?.data).toEqual([
      { canonicalUrl: 'https://example.com/one' },
      { canonicalUrl: 'https://example.com/two' },
    ])
    expect(peekHabitPatterns(april, 'chrome:Default')).toBe(habitPatterns)
    expect(peekInterruptedHabits('chrome:Default')).toBe(interruptedHabits)
    expect(peekDiscoveryTrend(april, 'chrome:Default', 'day')).toBe(
      discoveryTrendDay,
    )
    expect(peekActivityMix(april, 'chrome:Default')).toBe(activityMix)

    await expect(getOnThisDay('chrome:Default')).resolves.toBe(onThisDay)
    await expect(
      getTopSites(april, 'chrome:Default', undefined, 1),
    ).resolves.toMatchObject({ data: [{ domain: 'example.com' }] })
    await expect(getTopSites(april, 'chrome:Default')).resolves.toMatchObject({
      data: [{ domain: 'example.com' }, { domain: 'docs.example.com' }],
    })
    await expect(getSearchEngineRanking(april, 'chrome:Default')).resolves.toBe(
      searchEngineRanking,
    )
    await expect(
      getTopSearchConcepts(april, 'chrome:Default', 1),
    ).resolves.toMatchObject({ data: [{ term: 'tauri' }] })
    await expect(
      getTopSearchConcepts(april, 'chrome:Default'),
    ).resolves.toMatchObject({ data: [{ term: 'tauri' }, { term: 'sqlite' }] })
    await expect(
      getQueryFamilies(april, 'chrome:Default', {
        page: 0,
        pageSize: 1,
      }),
    ).resolves.toMatchObject({ data: { families: [{ id: 'family-a' }] } })
    await expect(
      getQueryFamilies(april, 'chrome:Default'),
    ).resolves.toMatchObject({
      data: {
        page: 0,
        pageSize: 10,
        families: [{ id: 'family-a' }, { id: 'family-b' }],
      },
    })
    await expect(
      getRefindPages(april, 'chrome:Default', 1),
    ).resolves.toMatchObject({
      data: [{ canonicalUrl: 'https://example.com/one' }],
    })
    await expect(
      getRefindPages(april, 'chrome:Default'),
    ).resolves.toMatchObject({
      data: [
        { canonicalUrl: 'https://example.com/one' },
        { canonicalUrl: 'https://example.com/two' },
      ],
    })
    await expect(getHabitPatterns(april, 'chrome:Default')).resolves.toBe(
      habitPatterns,
    )
    await expect(getInterruptedHabits('chrome:Default')).resolves.toBe(
      interruptedHabits,
    )
    await expect(getActivityMix(april, 'chrome:Default')).resolves.toBe(
      activityMix,
    )
    expect(callMock).not.toHaveBeenCalled()
  })

  test('reuses cached secondary overview sections and keeps browsing rhythm direct', async () => {
    const {
      getBreadthIndex,
      getBrowsingRhythm,
      getCompareSets,
      getFrictionSignals,
      getMultiBrowserDiff,
      getObservedInteractions,
      getPathFlows,
      getReopenedInvestigations,
      getSearchEffectiveness,
      getStableSources,
      peekBreadthIndex,
      peekCompareSets,
      peekDiscoveryTrend,
      peekFrictionSignals,
      peekMultiBrowserDiff,
      peekObservedInteractions,
      peekPathFlows,
      peekReopenedInvestigations,
      peekSearchEffectiveness,
      peekStableSources,
    } = await import('./api/overview')
    const { writeOverviewCache } = await import('./api/shared')
    const stableSources = testSection('stable-sources', [
      { domain: 'example.com' },
    ])
    const searchEffectiveness = testSection('search-effectiveness', {
      engine: 'google',
    })
    const frictionSignals = testSection('friction-signals', [
      { id: 'friction-a' },
    ])
    const reopenedInvestigations = testSection('reopened-investigations', [
      { id: 'reopened-a' },
    ])
    const discoveryTrendWeek = testSection('discovery-trend', { points: [] })
    const breadthIndex = testSection('breadth-index', { score: 0.7 })
    const pathFlows = testSection('path-flows', [
      { id: 'path-a' },
      { id: 'path-b' },
    ])
    const compareSets = testSection('compare-sets', [{ id: 'compare-a' }])
    const multiBrowserDiff = testSection('multi-browser-diff', {
      differences: [],
    })
    const observedInteractions = testSection('observed-interactions', [
      { id: 'observed-a' },
    ])

    writeOverviewCache(april, 'chrome:Default', () => ({
      secondary: {
        stableSources,
        searchEffectiveness,
        frictionSignals,
        reopenedInvestigations,
        discoveryTrendWeek,
        breadthIndex,
        pathFlows,
        compareSets,
        multiBrowserDiff,
        observedInteractions,
        timings: [],
        totalDurationMs: 0,
      } as unknown as CoreIntelligenceSecondaryOverview,
    }))

    expect(peekStableSources(april, 'chrome:Default')).toBe(stableSources)
    expect(peekSearchEffectiveness(april, 'chrome:Default')).toBe(
      searchEffectiveness,
    )
    expect(peekFrictionSignals(april, 'chrome:Default')).toBe(frictionSignals)
    expect(peekReopenedInvestigations(april, 'chrome:Default')).toBe(
      reopenedInvestigations,
    )
    expect(peekDiscoveryTrend(april, 'chrome:Default', 'week')).toBe(
      discoveryTrendWeek,
    )
    expect(peekDiscoveryTrend(april, 'chrome:Default')).toBe(discoveryTrendWeek)
    expect(peekBreadthIndex(april, 'chrome:Default')).toBe(breadthIndex)
    expect(peekPathFlows(april, 'chrome:Default', 3, 1)?.data).toEqual([
      { id: 'path-a' },
    ])
    expect(peekPathFlows(april, 'chrome:Default')?.data).toEqual([
      { id: 'path-a' },
      { id: 'path-b' },
    ])
    expect(peekCompareSets(april, 'chrome:Default')).toBe(compareSets)
    expect(peekMultiBrowserDiff(april)).toBe(multiBrowserDiff)
    expect(peekObservedInteractions(april, 'chrome:Default')).toBe(
      observedInteractions,
    )

    await expect(getStableSources(april, 'chrome:Default')).resolves.toBe(
      stableSources,
    )
    await expect(getSearchEffectiveness(april, 'chrome:Default')).resolves.toBe(
      searchEffectiveness,
    )
    callMock.mockResolvedValueOnce({ engine: 'duckduckgo' })
    await expect(
      getSearchEffectiveness(april, 'chrome:Default', 'duckduckgo'),
    ).resolves.toMatchObject({
      data: { engine: 'duckduckgo' },
      meta: { sectionId: 'search-effectiveness' },
    })
    await expect(getFrictionSignals(april, 'chrome:Default')).resolves.toBe(
      frictionSignals,
    )
    await expect(
      getReopenedInvestigations(april, 'chrome:Default'),
    ).resolves.toBe(reopenedInvestigations)
    await expect(getBreadthIndex(april, 'chrome:Default')).resolves.toBe(
      breadthIndex,
    )
    await expect(
      getPathFlows(april, 'chrome:Default', 3, 1),
    ).resolves.toMatchObject({ data: [{ id: 'path-a' }] })
    await expect(getPathFlows(april, 'chrome:Default')).resolves.toMatchObject({
      data: [{ id: 'path-a' }, { id: 'path-b' }],
    })
    await expect(getCompareSets(april, 'chrome:Default')).resolves.toBe(
      compareSets,
    )
    await expect(getMultiBrowserDiff(april)).resolves.toBe(multiBrowserDiff)
    await expect(
      getObservedInteractions(april, 'chrome:Default'),
    ).resolves.toBe(observedInteractions)
    expect(callMock).toHaveBeenCalledWith('get_search_effectiveness', {
      request: {
        dateRange: april,
        profileId: 'chrome:Default',
        engine: 'duckduckgo',
      },
    })
    callMock.mockClear()

    callMock.mockResolvedValueOnce({ buckets: [] })
    await expect(
      getBrowsingRhythm(april, 'chrome:Default', 'work'),
    ).resolves.toMatchObject({
      data: { buckets: [] },
      meta: { sectionId: 'browsing-rhythm' },
    })
    expect(callMock).toHaveBeenCalledWith('get_browsing_rhythm', {
      request: {
        dateRange: april,
        profileId: 'chrome:Default',
        category: 'work',
      },
    })
  })

  test('invokes direct reads when cached overview filters cannot satisfy them', async () => {
    const { getDiscoveryTrend, getTopSites } = await import('./api/overview')
    const { writeOverviewCache } = await import('./api/shared')

    writeOverviewCache(april, 'chrome:Default', () => ({
      primary: {
        topSites: testSection('top-sites', [{ domain: 'overview.example' }]),
      } as unknown as CoreIntelligencePrimaryOverview,
      secondary: {
        discoveryTrendWeek: testSection('discovery-trend', {
          points: ['week'],
        }),
      } as unknown as CoreIntelligenceSecondaryOverview,
    }))

    callMock
      .mockResolvedValueOnce([{ domain: 'duration.example' }])
      .mockResolvedValueOnce({ points: ['month'] })

    await expect(
      getTopSites(april, 'chrome:Default', 'duration', 1),
    ).resolves.toMatchObject({
      data: [{ domain: 'duration.example' }],
      meta: { sectionId: 'top-sites' },
    })
    await expect(
      getDiscoveryTrend(april, 'chrome:Default', 'month'),
    ).resolves.toMatchObject({
      data: { points: ['month'] },
      meta: { sectionId: 'discovery-trend' },
    })

    expect(callMock).toHaveBeenNthCalledWith(1, 'get_top_sites', {
      request: {
        dateRange: april,
        profileId: 'chrome:Default',
        sortBy: 'duration',
        limit: 1,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(2, 'get_discovery_trend', {
      request: {
        dateRange: april,
        profileId: 'chrome:Default',
        granularity: 'month',
      },
    })
  })

  test('invokes primary reads with default filters when overview cache is cold', async () => {
    const {
      getQueryFamilies,
      getRefindPages,
      getSearchQueries,
      getTopSearchConcepts,
      getTopSites,
    } = await import('./api/overview')

    callMock
      .mockResolvedValueOnce([{ domain: 'cold.example' }])
      .mockResolvedValueOnce([{ term: 'cold concept' }])
      .mockResolvedValueOnce({
        page: 0,
        pageSize: 20,
        total: 0,
        families: [],
      })
      .mockResolvedValueOnce([{ canonicalUrl: 'https://cold.example' }])
      .mockResolvedValueOnce({
        rows: [],
        page: 0,
        pageSize: 20,
        total: 0,
        facets: {
          browsers: [],
          engines: [],
          domains: [],
        },
      })

    await expect(getTopSites(april)).resolves.toMatchObject({
      data: [{ domain: 'cold.example' }],
    })
    await expect(getTopSearchConcepts(april)).resolves.toMatchObject({
      data: [{ term: 'cold concept' }],
    })
    await expect(getQueryFamilies(april)).resolves.toMatchObject({
      data: { page: 0, pageSize: 20, families: [] },
    })
    await expect(getRefindPages(april)).resolves.toMatchObject({
      data: [{ canonicalUrl: 'https://cold.example' }],
    })
    await expect(getSearchQueries(april)).resolves.toMatchObject({
      data: { rows: [], page: 0, pageSize: 20 },
    })

    expect(callMock).toHaveBeenNthCalledWith(1, 'get_top_sites', {
      request: {
        dateRange: april,
        profileId: undefined,
        sortBy: undefined,
        limit: undefined,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(2, 'get_top_search_concepts', {
      request: { dateRange: april, profileId: undefined, limit: undefined },
    })
    expect(callMock).toHaveBeenNthCalledWith(3, 'get_query_families', {
      request: {
        dateRange: april,
        profileId: undefined,
        page: 0,
        pageSize: 20,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(4, 'get_refind_pages', {
      request: { dateRange: april, profileId: undefined, limit: undefined },
    })
    expect(callMock).toHaveBeenNthCalledWith(5, 'get_search_queries', {
      request: {
        dateRange: april,
        profileId: undefined,
        browserKind: undefined,
        engine: undefined,
        domain: undefined,
        query: undefined,
        sort: undefined,
        page: 0,
        pageSize: 20,
      },
    })
  })

  test('falls back to cached section reads when overview filters cannot satisfy peeks', async () => {
    const {
      peekDiscoveryTrend,
      peekPathFlows,
      peekQueryFamilies,
      peekSearchEffectiveness,
      peekSearchQueries,
      peekTopSites,
    } = await import('./api/overview')
    const { writeCachedReadResult, writeOverviewCache } =
      await import('./api/shared')
    const topSites = testSection('top-sites', [{ domain: 'fresh-sort' }])
    const queryFamilies = testSection('search-activity', {
      page: 1,
      pageSize: 2,
      total: 3,
      families: [{ id: 'page-one' }],
    })
    const searchEffectiveness = testSection('search-effectiveness', {
      engine: 'google',
    })
    const discoveryTrend = testSection('discovery-trend', { points: ['month'] })
    const pathFlows = testSection('path-flows', [{ id: 'four-step' }])
    const defaultQueryFamilies = testSection('search-activity', {
      page: 0,
      pageSize: 20,
      total: 1,
      families: [{ id: 'default-page' }],
    })
    const defaultSearchQueries = testSection('search-activity', {
      rows: [{ query: 'default search' }],
      page: 0,
      pageSize: 20,
      total: 1,
      facets: { browsers: [], engines: [], domains: [] },
    })

    writeOverviewCache(april, 'chrome:Default', () => ({
      primary: {
        topSites: testSection('top-sites', [{ domain: 'overview' }]),
        queryFamilies: testSection('search-activity', {
          page: 0,
          pageSize: 1,
          total: 1,
          families: [{ id: 'overview' }],
        }),
      } as unknown as CoreIntelligencePrimaryOverview,
      secondary: {
        searchEffectiveness: testSection('search-effectiveness', {
          engine: 'all',
        }),
        discoveryTrendWeek: testSection('discovery-trend', {
          points: ['week'],
        }),
        pathFlows: testSection('path-flows', [{ id: 'three-step' }]),
      } as unknown as CoreIntelligenceSecondaryOverview,
    }))
    writeCachedReadResult(
      'get_top_sites',
      {
        request: {
          dateRange: april,
          profileId: 'chrome:Default',
          sortBy: 'title',
          limit: 5,
        },
      },
      topSites,
    )
    writeCachedReadResult(
      'get_query_families',
      {
        request: {
          dateRange: april,
          profileId: 'chrome:Default',
          page: 1,
          pageSize: 2,
        },
      },
      queryFamilies,
    )
    writeCachedReadResult(
      'get_query_families',
      {
        request: {
          dateRange: april,
          profileId: null,
          page: 0,
          pageSize: 20,
        },
      },
      defaultQueryFamilies,
    )
    writeCachedReadResult(
      'get_search_queries',
      {
        request: {
          dateRange: april,
          profileId: undefined,
          browserKind: undefined,
          engine: undefined,
          domain: undefined,
          query: undefined,
          sort: undefined,
          page: 0,
          pageSize: 20,
        },
      },
      defaultSearchQueries,
    )
    writeCachedReadResult(
      'get_search_effectiveness',
      {
        request: {
          dateRange: april,
          profileId: 'chrome:Default',
          engine: 'google',
        },
      },
      searchEffectiveness,
    )
    writeCachedReadResult(
      'get_discovery_trend',
      {
        request: {
          dateRange: april,
          profileId: 'chrome:Default',
          granularity: 'month',
        },
      },
      discoveryTrend,
    )
    writeCachedReadResult(
      'get_path_flows',
      {
        request: {
          dateRange: april,
          profileId: 'chrome:Default',
          stepCount: 4,
          limit: 1,
        },
      },
      pathFlows,
    )

    expect(peekTopSites(april, 'chrome:Default', 'title', 5)).toBe(topSites)
    expect(
      peekQueryFamilies(april, 'chrome:Default', { page: 1, pageSize: 2 }),
    ).toBe(queryFamilies)
    expect(peekQueryFamilies(april, null)).toBe(defaultQueryFamilies)
    expect(peekSearchQueries(april)).toBe(defaultSearchQueries)
    expect(peekSearchEffectiveness(april, 'chrome:Default', 'google')).toBe(
      searchEffectiveness,
    )
    expect(peekDiscoveryTrend(april, 'chrome:Default', 'month')).toBe(
      discoveryTrend,
    )
    expect(peekPathFlows(april, 'chrome:Default', 4, 1)).toBe(pathFlows)
    expect(callMock).not.toHaveBeenCalled()
  })

  test('overview loaders normalize payloads, seed section caches, and preserve force reads', async () => {
    const {
      getDiscoveryTrend,
      loadIntelligencePrimaryOverview,
      loadIntelligenceSecondaryOverview,
      peekPathFlows,
      peekTopSites,
    } = await import('./api/overview')
    const rawPrimary = {
      topSites: [{ registrableDomain: 'example.com' }],
      interruptedHabits: [{ id: 'interrupted-a' }],
      timings: [{ sectionId: 'top-sites', durationMs: 3 }],
      totalDurationMs: 11,
    }
    const rawSecondary = {
      stableSources: [{ domain: 'example.com' }],
      discoveryTrendWeek: { points: [{ label: 'week-1' }] },
      pathFlows: [{ id: 'path-a' }],
      multiBrowserDiff: { profiles: [] },
      timings: [{ sectionId: 'stable-sources', durationMs: 5 }],
      totalDurationMs: 17,
    }

    callMock.mockResolvedValueOnce(rawPrimary)
    await expect(
      loadIntelligencePrimaryOverview(april, 'chrome:Default'),
    ).resolves.toMatchObject({
      topSites: { data: [{ registrableDomain: 'example.com' }] },
      totalDurationMs: 11,
    })
    expect(callMock).toHaveBeenCalledWith('get_intelligence_primary_overview', {
      request: { dateRange: april, profileId: 'chrome:Default' },
    })
    expect(
      peekTopSites(april, 'chrome:Default', 'visit_count', 40),
    ).toMatchObject({
      data: [{ registrableDomain: 'example.com' }],
    })

    callMock.mockResolvedValueOnce(rawSecondary)
    await expect(
      loadIntelligenceSecondaryOverview(april, 'chrome:Default'),
    ).resolves.toMatchObject({
      stableSources: { data: [{ domain: 'example.com' }] },
      totalDurationMs: 17,
    })
    expect(callMock).toHaveBeenCalledWith(
      'get_intelligence_secondary_overview',
      {
        request: { dateRange: april, profileId: 'chrome:Default' },
      },
    )
    expect(peekPathFlows(april, 'chrome:Default', 3, 15)).toMatchObject({
      data: [{ id: 'path-a' }],
    })
    await expect(
      getDiscoveryTrend(april, 'chrome:Default', 'week'),
    ).resolves.toMatchObject({ data: { points: [{ label: 'week-1' }] } })

    callMock.mockResolvedValueOnce({ points: [{ label: 'forced' }] })
    await expect(
      getDiscoveryTrend(april, 'chrome:Default', 'week', { force: true }),
    ).resolves.toMatchObject({ data: { points: [{ label: 'forced' }] } })
    expect(callMock).toHaveBeenLastCalledWith('get_discovery_trend', {
      request: {
        dateRange: april,
        profileId: 'chrome:Default',
        granularity: 'week',
      },
    })
  })

  test('rebuild commands keep the typed command envelopes stable', async () => {
    const { queueCoreIntelligenceRebuild, runCoreIntelligenceNow } =
      await import('./api/overview')

    callMock
      .mockResolvedValueOnce({ runId: 42, notes: ['rebuilt'] })
      .mockResolvedValueOnce({ jobId: 7, notes: ['queued'] })

    await expect(
      runCoreIntelligenceNow({
        profileId: 'chrome:Default',
        fullRebuild: true,
        limit: 500,
      }),
    ).resolves.toEqual({ runId: 42, notes: ['rebuilt'] })
    await expect(
      queueCoreIntelligenceRebuild({
        profileId: null,
        fullRebuild: false,
        limit: null,
      }),
    ).resolves.toEqual({ jobId: 7, notes: ['queued'] })

    expect(callMock).toHaveBeenNthCalledWith(1, 'run_core_intelligence_now', {
      request: {
        profileId: 'chrome:Default',
        fullRebuild: true,
        limit: 500,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(
      2,
      'queue_core_intelligence_rebuild',
      {
        request: {
          profileId: null,
          fullRebuild: false,
          limit: null,
        },
      },
    )
  })
})
