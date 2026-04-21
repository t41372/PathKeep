/**
 * @file overview-peek.ts
 * @description Exposes warm-cache peek helpers for overview-backed Core Intelligence sections.
 * @module lib/core-intelligence/api
 *
 * ## 職責
 * - 提供不觸發新請求的 warm-cache `peek*` 讀取。
 * - 優先命中 overview cache，再回退到 section read cache。
 * - 在支援 slice/first-page reuse 的情況下回傳 cache 子集，避免不必要 request。
 *
 * ## 不負責
 * - 不發起任何後端請求。
 * - 不修改 overview cache。
 * - 不提供 Promise-based section read wrappers。
 *
 * ## 依賴關係
 * - 依賴 `shared.ts` 的 cached read/overview cache primitives。
 * - 和 `overview-loaders.ts` 協作，吃 overview load 時 seed 進來的 cache。
 *
 * ## 性能備注
 * - 所有 `peek*` 都是同步 warm-cache 查詢，目的是讓 staged UI 回訪或 tab 切換先吃現成資料而不是重新走 IPC。
 */

import type {
  ActivityMix,
  BreadthIndex,
  BrowserDiff,
  CompareSet,
  CoreIntelligenceSectionResult,
  DateRange,
  DigestSummary,
  DiscoveryTrend,
  FrictionSignal,
  HabitPattern,
  InterruptedHabit,
  ObservedInteraction,
  PathFlow,
  QueryFamilyResult,
  RefindPage,
  ReopenedInvestigation,
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
  peekCachedReadResult,
} from './shared'

/**
 * Looks up a cached section-result envelope without triggering a fetch.
 *
 * This is the lowest-level escape hatch used by the more specific `peek*`
 * helpers when no overview-level shortcut is available.
 */
export function peekCachedSectionRequest<T>(
  command: string,
  request: Record<string, unknown>,
) {
  return peekCachedReadResult<CoreIntelligenceSectionResult<T>>(command, {
    request,
  })
}

/**
 * Returns the cached primary overview for the requested scope if it is already warm.
 *
 * Callers use this to decide whether the first overview render can skip cold
 * skeletons for the primary summary surface.
 */
export function peekIntelligencePrimaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return cachedPrimaryOverview(dateRange, profileId)
}

/**
 * Returns the cached secondary overview for the requested scope if it is already warm.
 *
 * This supports staged overview revisit paths that should restore secondary
 * cards immediately and revalidate later.
 */
export function peekIntelligenceSecondaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return cachedSecondaryOverview(dateRange, profileId)
}

/**
 * Returns the cached digest summary for the requested scope.
 *
 * Digest summary is the most reused overview snippet, so it checks the
 * overview snapshot first and only then falls back to the section cache.
 */
export function peekDigestSummary(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedPrimaryOverview(dateRange, profileId)?.digestSummary ??
    peekCachedSectionRequest<DigestSummary>('get_digest_summary', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached top sites, slicing the warm overview payload when possible.
 *
 * The slice behavior avoids a second request when the caller only needs the
 * default visit-count ordering or a smaller limit.
 */
export function peekTopSites(
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
    return {
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    }
  }

  return peekCachedSectionRequest<TopSite[]>('get_top_sites', {
    dateRange,
    profileId,
    sortBy,
    limit,
  })
}

/**
 * Returns the cached search-engine ranking for the requested scope.
 */
export function peekSearchEngineRanking(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedPrimaryOverview(dateRange, profileId)?.searchEngineRanking ??
    peekCachedSectionRequest<EngineRanking[]>('get_search_engine_ranking', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached top search concepts, slicing the overview payload when possible.
 *
 * The limit-aware slice keeps the search activity card responsive on revisit.
 */
export function peekTopSearchConcepts(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.topSearchConcepts
  if (cached && (!limit || limit <= cached.data.length)) {
    return {
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    }
  }

  return peekCachedSectionRequest<SearchConcept[]>('get_top_search_concepts', {
    dateRange,
    profileId,
    limit,
  })
}

/**
 * Returns cached search-query browser results for the requested filters if present.
 *
 * Unlike the overview cards, this surface has more filter combinations, so it
 * only reads from the section cache.
 */
export function peekSearchQueries(
  dateRange: DateRange,
  options?: {
    profileId?: string | null
    browserKind?: string | null
    engine?: string | null
    domain?: string | null
    query?: string | null
    sort?: SearchQuerySort
    pagination?: { page: number; pageSize: number }
  },
) {
  return peekCachedSectionRequest<SearchQueryListResult>('get_search_queries', {
    dateRange,
    profileId: options?.profileId,
    browserKind: options?.browserKind,
    engine: options?.engine,
    domain: options?.domain,
    query: options?.query,
    sort: options?.sort,
    page: options?.pagination?.page ?? 0,
    pageSize: options?.pagination?.pageSize ?? 20,
  })
}

/**
 * Returns cached query families, slicing the overview first-page payload when possible.
 *
 * This lets the overview card reuse its already loaded page-zero families.
 */
export function peekQueryFamilies(
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
    return {
      ...cached,
      data: {
        ...cached.data,
        page: pagination?.page ?? 0,
        pageSize: pagination?.pageSize ?? cached.data.pageSize,
        families: cached.data.families.slice(0, pagination?.pageSize ?? 10),
      },
    }
  }

  return peekCachedSectionRequest<QueryFamilyResult>('get_query_families', {
    dateRange,
    profileId,
    page: pagination?.page ?? 0,
    pageSize: pagination?.pageSize ?? 20,
  })
}

/**
 * Returns cached refind pages, slicing the overview payload when possible.
 */
export function peekRefindPages(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  const cached = cachedPrimaryOverview(dateRange, profileId)?.refindPages
  if (cached && (!limit || limit <= cached.data.length)) {
    return {
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    }
  }

  return peekCachedSectionRequest<RefindPage[]>('get_refind_pages', {
    dateRange,
    profileId,
    limit,
  })
}

/**
 * Returns cached habit patterns for the requested overview scope.
 */
export function peekHabitPatterns(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedPrimaryOverview(dateRange, profileId)?.habitPatterns ??
    peekCachedSectionRequest<HabitPattern[]>('get_habit_patterns', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached interrupted habits keyed only by profile scope.
 *
 * This prefers the overview snapshot because interrupted habits are profile-wide
 * and not tied to one date range card instance.
 */
export function peekInterruptedHabits(profileId?: string | null) {
  return (
    cachedPrimarySectionForProfile(
      profileId,
      (overview) => overview.interruptedHabits,
    ) ??
    peekCachedSectionRequest<InterruptedHabit[]>('get_interrupted_habits', {
      profileId,
    })
  )
}

/**
 * Returns cached stable sources for the requested scope.
 */
export function peekStableSources(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedSecondaryOverview(dateRange, profileId)?.stableSources ??
    peekCachedSectionRequest<StableSource[]>('get_stable_sources', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached search effectiveness, preferring the overview snapshot for the default engine scope.
 *
 * Engine-specific views bypass the overview shortcut because they are not part
 * of the canonical secondary overview payload.
 */
export function peekSearchEffectiveness(
  dateRange: DateRange,
  profileId?: string | null,
  engine?: string,
) {
  if (!engine) {
    const cached = cachedSecondaryOverview(
      dateRange,
      profileId,
    )?.searchEffectiveness
    if (cached) {
      return cached
    }
  }

  return peekCachedSectionRequest<SearchEffectiveness>(
    'get_search_effectiveness',
    {
      dateRange,
      profileId,
      engine,
    },
  )
}

/**
 * Returns cached friction signals for the requested scope.
 */
export function peekFrictionSignals(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedSecondaryOverview(dateRange, profileId)?.frictionSignals ??
    peekCachedSectionRequest<FrictionSignal[]>('get_friction_signals', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached reopened investigations for the requested scope.
 */
export function peekReopenedInvestigations(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedSecondaryOverview(dateRange, profileId)?.reopenedInvestigations ??
    peekCachedSectionRequest<ReopenedInvestigation[]>(
      'get_reopened_investigations',
      {
        dateRange,
        profileId,
      },
    )
  )
}

/**
 * Returns cached discovery trend for day or week granularity when available.
 *
 * The overview stores day and week trends in different snapshots, so this
 * helper selects the right cache source before falling back to the section cache.
 */
export function peekDiscoveryTrend(
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
    return cached
  }

  return peekCachedSectionRequest<DiscoveryTrend>('get_discovery_trend', {
    dateRange,
    profileId,
    granularity,
  })
}

/**
 * Returns cached activity mix for the requested scope.
 */
export function peekActivityMix(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedPrimaryOverview(dateRange, profileId)?.activityMix ??
    peekCachedSectionRequest<ActivityMix>('get_activity_mix', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached breadth index for the requested scope.
 */
export function peekBreadthIndex(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedSecondaryOverview(dateRange, profileId)?.breadthIndex ??
    peekCachedSectionRequest<BreadthIndex>('get_breadth_index', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached path flows, slicing the default three-step overview payload when possible.
 */
export function peekPathFlows(
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
    return {
      ...cached,
      data: cached.data.slice(0, limit ?? cached.data.length),
    }
  }

  return peekCachedSectionRequest<PathFlow[]>('get_path_flows', {
    dateRange,
    profileId,
    stepCount,
    limit,
  })
}

/**
 * Returns cached compare sets for the requested scope.
 */
export function peekCompareSets(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedSecondaryOverview(dateRange, profileId)?.compareSets ??
    peekCachedSectionRequest<CompareSet[]>('get_compare_sets', {
      dateRange,
      profileId,
    })
  )
}

/**
 * Returns cached multi-browser diff for the requested date range.
 *
 * This surface is date-range wide and does not vary by profile, so it uses the
 * date-range secondary cache shortcut.
 */
export function peekMultiBrowserDiff(dateRange: DateRange) {
  return (
    cachedSecondarySectionForDateRange(
      dateRange,
      (overview) => overview.multiBrowserDiff,
    ) ??
    peekCachedSectionRequest<BrowserDiff>('get_multi_browser_diff', {
      dateRange,
    })
  )
}

/**
 * Returns cached observed interactions for the requested scope.
 */
export function peekObservedInteractions(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return (
    cachedSecondaryOverview(dateRange, profileId)?.observedInteractions ??
    peekCachedSectionRequest<ObservedInteraction[]>(
      'get_observed_interactions',
      {
        dateRange,
        profileId,
      },
    )
  )
}
