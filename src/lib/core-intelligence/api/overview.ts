/**
 * Overview-focused Core Intelligence API wrappers.
 *
 * Why this file exists:
 * - Overview sections and overview cache behavior share one ownership boundary
 *   after M10 splits the old API mega-file.
 */

import type {
  ActivityMix,
  ActivityMixTrend,
  BreadthIndex,
  BrowserDiff,
  CompareSet,
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceQueueReport,
  CoreIntelligenceRebuildReport,
  CoreIntelligenceRebuildRequest,
  CoreIntelligenceSecondaryOverview,
  DateRange,
  DigestSummary,
  DiscoveryTrend,
  FrictionSignal,
  HabitPattern,
  InterruptedHabit,
  ObservedInteraction,
  OnThisDayEntry,
  PathFlow,
  QueryFamilyResult,
  RefindPage,
  ReopenedInvestigation,
  RhythmHeatmap,
  SearchConcept,
  SearchEffectiveness,
  SearchQueryListResult,
  SearchQuerySort,
  StableSource,
  TopSite,
  EngineRanking,
} from '../types'
import {
  cachedPrimaryOverview,
  cachedPrimarySectionForProfile,
  cachedSecondaryOverview,
  cachedSecondarySectionForDateRange,
  clearOverviewCache,
  formatLocalDateKey,
  invokeRequest,
  invokeSectionArgs,
  invokeSectionRequest,
  normalizePrimaryOverview,
  normalizeSecondaryOverview,
  writeOverviewCache,
} from './shared'

export function runCoreIntelligenceNow(
  request: CoreIntelligenceRebuildRequest,
) {
  return invokeRequest<CoreIntelligenceRebuildReport, Record<string, unknown>>(
    'run_core_intelligence_now',
    { ...request },
  )
}

export function queueCoreIntelligenceRebuild(
  request: CoreIntelligenceRebuildRequest,
) {
  return invokeRequest<CoreIntelligenceQueueReport, Record<string, unknown>>(
    'queue_core_intelligence_rebuild',
    { ...request },
  )
}

export function loadIntelligencePrimaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    CoreIntelligencePrimaryOverview,
    { dateRange: DateRange; profileId?: string | null }
  >('get_intelligence_primary_overview', {
    dateRange,
    profileId,
  }).then((result) => {
    const normalized = normalizePrimaryOverview(dateRange, result)
    writeOverviewCache(dateRange, profileId, (current) => ({
      ...current,
      primary: normalized,
    }))
    return normalized
  })
}

export function loadIntelligenceSecondaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    CoreIntelligenceSecondaryOverview,
    { dateRange: DateRange; profileId?: string | null }
  >('get_intelligence_secondary_overview', {
    dateRange,
    profileId,
  }).then((result) => {
    const normalized = normalizeSecondaryOverview(dateRange, result)
    writeOverviewCache(dateRange, profileId, (current) => ({
      ...current,
      secondary: normalized,
    }))
    return normalized
  })
}

export function clearIntelligenceOverviewCache() {
  clearOverviewCache()
}

export function peekIntelligencePrimaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return cachedPrimaryOverview(dateRange, profileId)
}

export function peekIntelligenceSecondaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return cachedSecondaryOverview(dateRange, profileId)
}

export function getDigestSummary(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.digestSummary
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    DigestSummary,
    { dateRange: DateRange; profileId?: string | null }
  >(
    'get_digest_summary',
    {
      dateRange,
      profileId,
    },
    'digest-summary',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getOnThisDay(profileId?: string | null) {
  const cached = cachedPrimarySectionForProfile(
    profileId,
    (overview) => overview.onThisDay,
  )
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionArgs<OnThisDayEntry[]>(
    'get_on_this_day',
    { profileId },
    'on-this-day',
    {
      kind: 'calendar-day-history',
      referenceDate: formatLocalDateKey(new Date()),
    },
  )
}

export function getTopSites(
  dateRange: DateRange,
  profileId?: string | null,
  sortBy?: string,
  limit?: number,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.topSites
  if (
    cached &&
    (!sortBy || sortBy === 'visit_count') &&
    (!limit || limit <= cached.data.length)
  ) {
    return Promise.resolve({
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    })
  }
  return invokeSectionRequest<
    TopSite[],
    {
      dateRange: DateRange
      profileId?: string | null
      sortBy?: string
      limit?: number
    }
  >(
    'get_top_sites',
    {
      dateRange,
      profileId,
      sortBy,
      limit,
    },
    'top-sites',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getSearchEngineRanking(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedPrimaryOverview(
    dateRange,
    profileId,
  )?.searchEngineRanking
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    EngineRanking[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_search_engine_ranking',
    {
      dateRange,
      profileId,
    },
    'search-activity',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getTopSearchConcepts(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.topSearchConcepts
  if (cached && (!limit || limit <= cached.data.length)) {
    return Promise.resolve({
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    })
  }
  return invokeSectionRequest<
    SearchConcept[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >(
    'get_top_search_concepts',
    {
      dateRange,
      profileId,
      limit,
    },
    'search-activity',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getSearchQueries(
  dateRange: DateRange,
  options?: {
    profileId?: string | null
    browserKind?: string | null
    engine?: string | null
    query?: string | null
    sort?: SearchQuerySort
    pagination?: { page: number; pageSize: number }
  },
) {
  return invokeSectionRequest<
    SearchQueryListResult,
    {
      dateRange: DateRange
      profileId?: string | null
      browserKind?: string | null
      engine?: string | null
      query?: string | null
      sort?: SearchQuerySort
      page: number
      pageSize: number
    }
  >(
    'get_search_queries',
    {
      dateRange,
      profileId: options?.profileId,
      browserKind: options?.browserKind,
      engine: options?.engine,
      query: options?.query,
      sort: options?.sort,
      page: options?.pagination?.page ?? 0,
      pageSize: options?.pagination?.pageSize ?? 20,
    },
    'search-activity',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getQueryFamilies(
  dateRange: DateRange,
  profileId?: string | null,
  pagination?: { page: number; pageSize: number },
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.queryFamilies
  if (
    cached &&
    (pagination?.page ?? 0) === 0 &&
    (pagination?.pageSize ?? 10) <= cached.data.pageSize
  ) {
    return Promise.resolve({
      ...cached,
      data: {
        ...cached.data,
        page: pagination?.page ?? 0,
        pageSize: pagination?.pageSize ?? cached.data.pageSize,
        families: cached.data.families.slice(0, pagination?.pageSize ?? 10),
      },
    })
  }
  return invokeSectionRequest<
    QueryFamilyResult,
    {
      dateRange: DateRange
      profileId?: string | null
      page: number
      pageSize: number
    }
  >(
    'get_query_families',
    {
      dateRange,
      profileId,
      page: pagination?.page ?? 0,
      pageSize: pagination?.pageSize ?? 20,
    },
    'search-activity',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getRefindPages(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.refindPages
  if (cached && (!limit || limit <= cached.data.length)) {
    return Promise.resolve({
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    })
  }
  return invokeSectionRequest<
    RefindPage[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >(
    'get_refind_pages',
    {
      dateRange,
      profileId,
      limit,
    },
    'refind-pages',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getHabitPatterns(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.habitPatterns
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    HabitPattern[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_habit_patterns',
    {
      dateRange,
      profileId,
    },
    'habits',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getInterruptedHabits(profileId?: string | null) {
  const cached = cachedPrimarySectionForProfile(
    profileId,
    (overview) => overview.interruptedHabits,
  )
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    InterruptedHabit[],
    { profileId?: string | null }
  >(
    'get_interrupted_habits',
    {
      profileId,
    },
    'habits',
    {
      kind: 'date-range',
      dateRange: { start: '', end: '' },
    },
  )
}

export function getStableSources(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedSecondaryOverview(dateRange, profileId)?.stableSources
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    StableSource[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_stable_sources',
    {
      dateRange,
      profileId,
    },
    'stable-sources',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getSearchEffectiveness(
  dateRange: DateRange,
  profileId?: string | null,
  engine?: string,
) {
  const cached = !engine
    ? cachedSecondaryOverview(dateRange, profileId)?.searchEffectiveness
    : null
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    SearchEffectiveness,
    {
      dateRange: DateRange
      profileId?: string | null
      engine?: string
    }
  >(
    'get_search_effectiveness',
    {
      dateRange,
      profileId,
      engine,
    },
    'search-effectiveness',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getFrictionSignals(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedSecondaryOverview(dateRange, profileId)?.frictionSignals
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    FrictionSignal[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_friction_signals',
    {
      dateRange,
      profileId,
    },
    'friction-signals',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getReopenedInvestigations(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedSecondaryOverview(
    dateRange,
    profileId,
  )?.reopenedInvestigations
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    ReopenedInvestigation[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_reopened_investigations',
    {
      dateRange,
      profileId,
    },
    'reopened-investigations',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getBrowsingRhythm(
  dateRange: DateRange,
  profileId?: string | null,
  category?: string,
) {
  return invokeSectionRequest<
    RhythmHeatmap,
    {
      dateRange: DateRange
      profileId?: string | null
      category?: string
    }
  >(
    'get_browsing_rhythm',
    {
      dateRange,
      profileId,
      category,
    },
    'browsing-rhythm',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getDiscoveryTrend(
  dateRange: DateRange,
  profileId?: string | null,
  granularity?: string,
) {
  const cached =
    granularity === 'day'
      ? cachedPrimaryOverview(dateRange, profileId)?.discoveryTrendDay
      : granularity === 'week' || granularity === undefined
        ? cachedSecondaryOverview(dateRange, profileId)?.discoveryTrendWeek
        : null
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    DiscoveryTrend,
    {
      dateRange: DateRange
      profileId?: string | null
      granularity?: string
    }
  >(
    'get_discovery_trend',
    {
      dateRange,
      profileId,
      granularity,
    },
    'discovery-trend',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getActivityMix(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.activityMix
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    ActivityMix,
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_activity_mix',
    {
      dateRange,
      profileId,
    },
    'activity-mix',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getActivityMixTrend(
  dateRange: DateRange,
  profileId?: string | null,
  granularity?: string,
) {
  return invokeRequest<
    ActivityMixTrend,
    {
      dateRange: DateRange
      profileId?: string | null
      granularity?: string
    }
  >('get_activity_mix_trend', {
    dateRange,
    profileId,
    granularity,
  })
}

export function getBreadthIndex(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedSecondaryOverview(dateRange, profileId)?.breadthIndex
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    BreadthIndex,
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_breadth_index',
    {
      dateRange,
      profileId,
    },
    'breadth-index',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getPathFlows(
  dateRange: DateRange,
  profileId?: string | null,
  stepCount?: number,
  limit?: number,
) {
  const cached = cachedSecondaryOverview(dateRange, profileId)?.pathFlows
  if (
    cached &&
    (stepCount ?? 3) === 3 &&
    (limit ?? cached.data.length) <= cached.data.length
  ) {
    return Promise.resolve({
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    })
  }
  return invokeSectionRequest<
    PathFlow[],
    {
      dateRange: DateRange
      profileId?: string | null
      stepCount?: number
      limit?: number
    }
  >(
    'get_path_flows',
    {
      dateRange,
      profileId,
      stepCount,
      limit,
    },
    'path-flows',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getCompareSets(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedSecondaryOverview(dateRange, profileId)?.compareSets
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    CompareSet[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_compare_sets',
    {
      dateRange,
      profileId,
    },
    'compare-sets',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getMultiBrowserDiff(dateRange: DateRange) {
  const cached = cachedSecondarySectionForDateRange(
    dateRange,
    (overview) => overview.multiBrowserDiff,
  )
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<BrowserDiff, { dateRange: DateRange }>(
    'get_multi_browser_diff',
    { dateRange },
    'multi-browser-diff',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getObservedInteractions(
  dateRange: DateRange,
  profileId?: string | null,
) {
  const cached = cachedSecondaryOverview(
    dateRange,
    profileId,
  )?.observedInteractions
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    ObservedInteraction[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_observed_interactions',
    {
      dateRange,
      profileId,
    },
    'observed-interactions',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}
