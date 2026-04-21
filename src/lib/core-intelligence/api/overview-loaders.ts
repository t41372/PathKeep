/**
 * @file overview-loaders.ts
 * @description Owns the overview-level Core Intelligence load paths and the cache seeding that keeps overview reads and section reads in sync.
 * @module lib/core-intelligence/api
 *
 * ## 職責
 * - 執行 primary/secondary overview 的 read-through load。
 * - 在 overview payload 命中後同步回填各 section read cache，避免同 scope 重複請求。
 * - 暴露 run/queue rebuild 與 overview cache clear 入口。
 *
 * ## 不負責
 * - 不提供 individual section `get*` read wrappers。
 * - 不提供 warm-cache `peek*` 查詢接口。
 * - 不定義 overview cache 的底層 shared primitives。
 *
 * ## 依賴關係
 * - 依賴 `shared.ts` 提供 invoke/cached-read/overview-cache primitives。
 * - 依賴 `../types` 提供 overview payload 與 rebuild request contracts。
 *
 * ## 性能備注
 * - overview load 會一次性 seed 多個 section cache，目的是降低同一個 date-range/profile scope 下的重複 IPC，而不是擴大 payload 邊界。
 */

import type {
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceQueueReport,
  CoreIntelligenceRebuildReport,
  CoreIntelligenceRebuildRequest,
  CoreIntelligenceSecondaryOverview,
  DateRange,
} from '../types'
import {
  clearOverviewCache,
  invokeCachedRead,
  invokeRequest,
  normalizePrimaryOverview,
  normalizeSecondaryOverview,
  writeCachedReadResult,
  writeOverviewCache,
} from './shared'

function requestArgs<T extends Record<string, unknown>>(request: T) {
  return { request }
}

function seedPrimaryOverviewReadCache(
  dateRange: DateRange,
  profileId: string | null | undefined,
  overview: CoreIntelligencePrimaryOverview,
) {
  writeCachedReadResult(
    'get_digest_summary',
    requestArgs({ dateRange, profileId }),
    overview.digestSummary,
  )
  writeCachedReadResult('get_on_this_day', { profileId }, overview.onThisDay)
  writeCachedReadResult(
    'get_top_sites',
    requestArgs({
      dateRange,
      profileId,
      sortBy: 'visit_count',
      limit: 40,
    }),
    overview.topSites,
  )
  writeCachedReadResult(
    'get_refind_pages',
    requestArgs({
      dateRange,
      profileId,
      limit: 5,
    }),
    overview.refindPages,
  )
  writeCachedReadResult(
    'get_search_engine_ranking',
    requestArgs({ dateRange, profileId }),
    overview.searchEngineRanking,
  )
  writeCachedReadResult(
    'get_top_search_concepts',
    requestArgs({
      dateRange,
      profileId,
      limit: 50,
    }),
    overview.topSearchConcepts,
  )
  writeCachedReadResult(
    'get_query_families',
    requestArgs({
      dateRange,
      profileId,
      page: 0,
      pageSize: 10,
    }),
    overview.queryFamilies,
  )
  writeCachedReadResult(
    'get_activity_mix',
    requestArgs({ dateRange, profileId }),
    overview.activityMix,
  )
  writeCachedReadResult(
    'get_discovery_trend',
    requestArgs({ dateRange, profileId, granularity: 'day' }),
    overview.discoveryTrendDay,
  )
  writeCachedReadResult(
    'get_habit_patterns',
    requestArgs({ dateRange, profileId }),
    overview.habitPatterns,
  )
  writeCachedReadResult(
    'get_interrupted_habits',
    { profileId },
    overview.interruptedHabits,
  )
}

function seedSecondaryOverviewReadCache(
  dateRange: DateRange,
  profileId: string | null | undefined,
  overview: CoreIntelligenceSecondaryOverview,
) {
  writeCachedReadResult(
    'get_stable_sources',
    requestArgs({ dateRange, profileId }),
    overview.stableSources,
  )
  writeCachedReadResult(
    'get_search_effectiveness',
    requestArgs({ dateRange, profileId, engine: undefined }),
    overview.searchEffectiveness,
  )
  writeCachedReadResult(
    'get_friction_signals',
    requestArgs({ dateRange, profileId }),
    overview.frictionSignals,
  )
  writeCachedReadResult(
    'get_reopened_investigations',
    requestArgs({ dateRange, profileId }),
    overview.reopenedInvestigations,
  )
  writeCachedReadResult(
    'get_discovery_trend',
    requestArgs({ dateRange, profileId, granularity: 'week' }),
    overview.discoveryTrendWeek,
  )
  writeCachedReadResult(
    'get_breadth_index',
    requestArgs({ dateRange, profileId }),
    overview.breadthIndex,
  )
  writeCachedReadResult(
    'get_path_flows',
    requestArgs({ dateRange, profileId, stepCount: 3, limit: 15 }),
    overview.pathFlows,
  )
  writeCachedReadResult(
    'get_compare_sets',
    requestArgs({ dateRange, profileId }),
    overview.compareSets,
  )
  writeCachedReadResult(
    'get_multi_browser_diff',
    requestArgs({ dateRange }),
    overview.multiBrowserDiff,
  )
  writeCachedReadResult(
    'get_observed_interactions',
    requestArgs({ dateRange, profileId }),
    overview.observedInteractions,
  )
}

/**
 * Runs a foreground Core Intelligence rebuild and returns the detailed report.
 *
 * This path is used when the caller needs the rebuild result now instead of
 * just queueing background work.
 */
export function runCoreIntelligenceNow(
  request: CoreIntelligenceRebuildRequest,
) {
  return invokeRequest<CoreIntelligenceRebuildReport, Record<string, unknown>>(
    'run_core_intelligence_now',
    { ...request },
  )
}

/**
 * Queues a Core Intelligence rebuild without blocking the caller on execution.
 *
 * This keeps heavy deterministic work off the current UI interaction path.
 */
export function queueCoreIntelligenceRebuild(
  request: CoreIntelligenceRebuildRequest,
) {
  return invokeRequest<CoreIntelligenceQueueReport, Record<string, unknown>>(
    'queue_core_intelligence_rebuild',
    { ...request },
  )
}

/**
 * Loads the primary overview and seeds matching section caches for the same scope.
 *
 * The extra cache writes are intentional: overview-first visits should not
 * immediately refetch digest/top-sites/query-family data through section reads.
 */
export function loadIntelligencePrimaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
  options?: { force?: boolean },
) {
  const args = requestArgs({ dateRange, profileId })
  return invokeCachedRead(
    'get_intelligence_primary_overview',
    args,
    (result) => {
      const normalized = normalizePrimaryOverview(
        dateRange,
        result as CoreIntelligencePrimaryOverview,
      )
      writeOverviewCache(dateRange, profileId, (current) => ({
        ...current,
        primary: normalized,
      }))
      seedPrimaryOverviewReadCache(dateRange, profileId, normalized)
      return normalized
    },
    options,
  )
}

/**
 * Loads the secondary overview and seeds matching section caches for the same scope.
 *
 * This keeps secondary-grid tabs and cards warm after the staged overview load
 * path finishes.
 */
export function loadIntelligenceSecondaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
  options?: { force?: boolean },
) {
  const args = requestArgs({ dateRange, profileId })
  return invokeCachedRead(
    'get_intelligence_secondary_overview',
    args,
    (result) => {
      const normalized = normalizeSecondaryOverview(
        dateRange,
        result as CoreIntelligenceSecondaryOverview,
      )
      writeOverviewCache(dateRange, profileId, (current) => ({
        ...current,
        secondary: normalized,
      }))
      seedSecondaryOverviewReadCache(dateRange, profileId, normalized)
      return normalized
    },
    options,
  )
}

/**
 * Clears the overview cache when a caller knows the current scoped summary is stale.
 *
 * This resets both primary and secondary overview memoized results together.
 */
export function clearIntelligenceOverviewCache() {
  clearOverviewCache()
}
