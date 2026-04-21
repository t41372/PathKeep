/**
 * @file overview-read-secondary.ts
 * @description Promise-based read wrappers for secondary overview-backed Core Intelligence surfaces.
 * @module lib/core-intelligence/api
 *
 * ## 職責
 * - 提供 secondary overview 相關的 `get*` deterministic reads。
 * - 在 warm overview cache 已存在時直接回傳 cached payload。
 *
 * ## 不負責
 * - 不處理 primary overview surfaces。
 * - 不提供同步 `peek*` helpers 或 overview load paths。
 *
 * ## 依賴關係
 * - 依賴 `shared.ts` 的 invoke helpers 與 secondary overview cache lookups。
 */

import type {
  BreadthIndex,
  BrowserDiff,
  CompareSet,
  DateRange,
  FrictionSignal,
  ObservedInteraction,
  PathFlow,
  ReopenedInvestigation,
  RhythmHeatmap,
  SearchEffectiveness,
  StableSource,
} from '../types'
import {
  cachedSecondaryOverview,
  cachedSecondarySectionForDateRange,
  invokeSectionRequest,
} from './shared'

/**
 * Returns stable sources, reusing the cached secondary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >('get_stable_sources', { dateRange, profileId }, 'stable-sources', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns search effectiveness, using the cached secondary overview for the default engine scope.
 */
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
    { dateRange: DateRange; profileId?: string | null; engine?: string }
  >(
    'get_search_effectiveness',
    { dateRange, profileId, engine },
    'search-effectiveness',
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns friction signals, reusing the cached secondary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >('get_friction_signals', { dateRange, profileId }, 'friction-signals', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns reopened investigations, reusing the cached secondary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >(
    'get_reopened_investigations',
    { dateRange, profileId },
    'reopened-investigations',
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns browsing rhythm for the requested scope.
 *
 * Browsing rhythm is intentionally always fetched directly because it is not
 * part of the overview cache boundary.
 */
export function getBrowsingRhythm(
  dateRange: DateRange,
  profileId?: string | null,
  category?: string,
) {
  return invokeSectionRequest<
    RhythmHeatmap,
    { dateRange: DateRange; profileId?: string | null; category?: string }
  >(
    'get_browsing_rhythm',
    { dateRange, profileId, category },
    'browsing-rhythm',
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns breadth index, reusing the cached secondary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >('get_breadth_index', { dateRange, profileId }, 'breadth-index', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns path flows, slicing the cached three-step overview payload when possible.
 */
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
    { dateRange, profileId, stepCount, limit },
    'path-flows',
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns compare sets, reusing the cached secondary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >('get_compare_sets', { dateRange, profileId }, 'compare-sets', {
    kind: 'date-range',
    dateRange,
  })
}

/**
 * Returns multi-browser diff, reusing the cached date-range overview payload when possible.
 */
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
    { kind: 'date-range', dateRange },
  )
}

/**
 * Returns observed interactions, reusing the cached secondary overview when possible.
 */
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
    { dateRange: DateRange; profileId?: string | null }
  >(
    'get_observed_interactions',
    { dateRange, profileId },
    'observed-interactions',
    { kind: 'date-range', dateRange },
  )
}
