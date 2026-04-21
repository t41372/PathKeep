/**
 * @file overview-read-primary.ts
 * @description Promise-based read wrappers for primary overview-backed Core Intelligence surfaces.
 * @module lib/core-intelligence/api
 *
 * ## 職責
 * - 提供 primary overview 相關的 `get*` deterministic reads。
 * - 在 warm overview cache 已存在時直接回傳 cached payload。
 *
 * ## 不負責
 * - 不處理 secondary overview surfaces。
 * - 不提供同步 `peek*` helpers 或 overview load paths。
 *
 * ## 依賴關係
 * - 依賴 `shared.ts` 的 invoke helpers 與 primary overview cache lookups。
 */

import type {
  ActivityMix,
  ActivityMixTrend,
  DateRange,
  DigestSummary,
  HabitPattern,
  InterruptedHabit,
  OnThisDayEntry,
  QueryFamilyResult,
  RefindPage,
  SearchConcept,
  SearchQueryListResult,
  SearchQuerySort,
  TopSite,
  EngineRanking,
} from '../types'
import {
  cachedPrimaryOverview,
  cachedPrimarySectionForProfile,
  formatLocalDateKey,
  invokeRequest,
  invokeSectionArgs,
  invokeSectionRequest,
} from './shared'

/**
 * Returns the digest summary, reusing the warm overview payload when available.
 */
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
  >('get_digest_summary', { dateRange, profileId }, 'digest-summary', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns On This Day entries, reusing the profile-scoped overview cache when available.
 */
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

/**
 * Returns top sites, slicing the cached overview payload when possible.
 */
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
  >('get_top_sites', { dateRange, profileId, sortBy, limit }, 'top-sites', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns search-engine ranking, reusing the cached primary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >('get_search_engine_ranking', { dateRange, profileId }, 'search-activity', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns top search concepts, slicing the cached overview payload when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null; limit?: number }
  >(
    'get_top_search_concepts',
    { dateRange, profileId, limit },
    'search-activity',
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns the search-query browser result for the requested filters.
 */
export function getSearchQueries(
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
  return invokeSectionRequest<
    SearchQueryListResult,
    {
      dateRange: DateRange
      profileId?: string | null
      browserKind?: string | null
      engine?: string | null
      domain?: string | null
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
      domain: options?.domain,
      query: options?.query,
      sort: options?.sort,
      page: options?.pagination?.page ?? 0,
      pageSize: options?.pagination?.pageSize ?? 20,
    },
    'search-activity',
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns query families, reusing the cached first page from the primary overview when possible.
 */
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
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns refind pages, slicing the cached overview payload when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null; limit?: number }
  >('get_refind_pages', { dateRange, profileId, limit }, 'refind-pages', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns habit patterns, reusing the cached primary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >('get_habit_patterns', { dateRange, profileId }, 'habits', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns interrupted habits, reusing the cached profile-scoped overview payload when possible.
 */
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
  >('get_interrupted_habits', { profileId }, 'habits', {
    kind: 'date-range',
    dateRange: { start: '', end: '' },
  })
}

/**
 * Returns activity mix, reusing the cached primary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >('get_activity_mix', { dateRange, profileId }, 'activity-mix', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns activity-mix trend for the requested scope.
 */
export function getActivityMixTrend(
  dateRange: DateRange,
  profileId?: string | null,
  granularity?: string,
) {
  return invokeRequest<
    ActivityMixTrend,
    { dateRange: DateRange; profileId?: string | null; granularity?: string }
  >('get_activity_mix_trend', {
    dateRange,
    profileId,
    granularity,
  })
}
