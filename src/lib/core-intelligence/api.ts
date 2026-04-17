/**
 * IPC invoke wrappers for Core Intelligence Tauri commands.
 *
 * Why this file exists:
 * - Centralizes all Core Intelligence IPC calls so routes and hooks don't scatter invoke logic.
 * - Each function maps 1:1 to a Tauri command defined in the implementation plan.
 * - Until the Rust backend is ready, these will throw "unavailable in preview" in browser mode,
 *   which is expected behavior aligned with the existing `invokeCommand` contract.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md`
 * - Backend commands implemented by Codex in `src-tauri/crates/vault-core/src/intelligence/`
 */

import { call } from '../backend-client/shared'
import type {
  DateRange,
  PaginationParams,
  CoreIntelligenceRebuildRequest,
  CoreIntelligenceRebuildReport,
  CoreIntelligenceQueueReport,
  DigestSummary,
  OnThisDayEntry,
  TopSite,
  DomainTrend,
  EngineRanking,
  SearchConcept,
  QueryFamilyResult,
  RefindPage,
  RefindExplanation,
  HabitPattern,
  InterruptedHabit,
  SessionListResult,
  SessionDetail,
  TrailListResult,
  TrailDetail,
  NavigationPath,
  HubPage,
  DomainDeepDive,
  StableSource,
  SearchEffectiveness,
  FrictionSignal,
  ReopenedInvestigation,
  RhythmHeatmap,
  DiscoveryTrend,
  ActivityMix,
  ActivityMixTrend,
  BreadthIndex,
  PathFlow,
  CompareSet,
  BrowserDiff,
  ObservedInteraction,
  Explanation,
} from './types'

function invokeRequest<TResponse, TRequest extends Record<string, unknown>>(
  command: string,
  request: TRequest,
) {
  return call<TResponse>(command, { request })
}

// ---------------------------------------------------------------------------
// Rebuild control
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 1.1 Digest Summary
// ---------------------------------------------------------------------------

export function getDigestSummary(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    DigestSummary,
    { dateRange: DateRange; profileId?: string | null }
  >('get_digest_summary', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 1.2 On This Day
// ---------------------------------------------------------------------------

export function getOnThisDay(profileId?: string | null) {
  return call<OnThisDayEntry[]>('get_on_this_day', { profileId })
}

// ---------------------------------------------------------------------------
// 2.1 Top Sites
// ---------------------------------------------------------------------------

export function getTopSites(
  dateRange: DateRange,
  profileId?: string | null,
  sortBy?: string,
  limit?: number,
) {
  return invokeRequest<
    TopSite[],
    {
      dateRange: DateRange
      profileId?: string | null
      sortBy?: string
      limit?: number
    }
  >('get_top_sites', {
    dateRange,
    profileId,
    sortBy,
    limit,
  })
}

export function getDomainTrend(domain: string, dateRange: DateRange) {
  return invokeRequest<
    DomainTrend,
    {
      registrableDomain: string
      dateRange: DateRange
    }
  >('get_domain_trend', { registrableDomain: domain, dateRange })
}

// ---------------------------------------------------------------------------
// 2.2 Search Activity
// ---------------------------------------------------------------------------

export function getSearchEngineRanking(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    EngineRanking[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_search_engine_ranking', {
    dateRange,
    profileId,
  })
}

export function getTopSearchConcepts(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    SearchConcept[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_top_search_concepts', {
    dateRange,
    profileId,
    limit,
  })
}

export function getQueryFamilies(
  dateRange: DateRange,
  profileId?: string | null,
  pagination?: PaginationParams,
) {
  return invokeRequest<
    QueryFamilyResult,
    {
      dateRange: DateRange
      profileId?: string | null
      page: number
      pageSize: number
    }
  >('get_query_families', {
    dateRange,
    profileId,
    page: pagination?.page ?? 0,
    pageSize: pagination?.pageSize ?? 20,
  })
}

// ---------------------------------------------------------------------------
// 2.3 Refind Pages
// ---------------------------------------------------------------------------

export function getRefindPages(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    RefindPage[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_refind_pages', {
    dateRange,
    profileId,
    limit,
  })
}

export function explainRefind(canonicalUrl: string) {
  return invokeRequest<RefindExplanation, { canonicalUrl: string }>(
    'explain_refind',
    { canonicalUrl },
  )
}

// ---------------------------------------------------------------------------
// 2.4 Habitual Visits
// ---------------------------------------------------------------------------

export function getHabitPatterns(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    HabitPattern[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_habit_patterns', {
    dateRange,
    profileId,
  })
}

export function getInterruptedHabits(profileId?: string | null) {
  return invokeRequest<InterruptedHabit[], { profileId?: string | null }>(
    'get_interrupted_habits',
    {
      profileId,
    },
  )
}

// ---------------------------------------------------------------------------
// 3.1 Sessions
// ---------------------------------------------------------------------------

export function getSessions(
  dateRange: DateRange,
  profileId?: string | null,
  pagination?: PaginationParams,
) {
  return invokeRequest<
    SessionListResult,
    {
      dateRange: DateRange
      profileId?: string | null
      page: number
      pageSize: number
    }
  >('get_sessions', {
    dateRange,
    profileId,
    page: pagination?.page ?? 0,
    pageSize: pagination?.pageSize ?? 20,
  })
}

export function getSessionDetail(sessionId: string) {
  return call<SessionDetail>('get_session_detail', { sessionId })
}

// ---------------------------------------------------------------------------
// 3.2 Search Trails
// ---------------------------------------------------------------------------

export function getSearchTrails(
  dateRange: DateRange,
  profileId?: string | null,
  engine?: string,
  pagination?: PaginationParams,
) {
  return invokeRequest<
    TrailListResult,
    {
      dateRange: DateRange
      profileId?: string | null
      engine?: string
      page: number
      pageSize: number
    }
  >('get_search_trails', {
    dateRange,
    profileId,
    engine,
    page: pagination?.page ?? 0,
    pageSize: pagination?.pageSize ?? 20,
  })
}

export function getTrailDetail(trailId: string) {
  return call<TrailDetail>('get_trail_detail', { trailId })
}

// ---------------------------------------------------------------------------
// 3.3 Navigation Path
// ---------------------------------------------------------------------------

export function getNavigationPath(visitId: number) {
  return call<NavigationPath>('get_navigation_path', { visitId })
}

export function getHubPages(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    HubPage[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_hub_pages', {
    dateRange,
    profileId,
    limit,
  })
}

// ---------------------------------------------------------------------------
// 4.1 Domain Deep Dive
// ---------------------------------------------------------------------------

export function getDomainDeepDive(
  domain: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    DomainDeepDive,
    {
      registrableDomain: string
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_domain_deep_dive', {
    registrableDomain: domain,
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.2 Stable Sources
// ---------------------------------------------------------------------------

export function getStableSources(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    StableSource[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_stable_sources', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.3 Search Effectiveness
// ---------------------------------------------------------------------------

export function getSearchEffectiveness(
  dateRange: DateRange,
  profileId?: string | null,
  engine?: string,
) {
  return invokeRequest<
    SearchEffectiveness,
    {
      dateRange: DateRange
      profileId?: string | null
      engine?: string
    }
  >('get_search_effectiveness', {
    dateRange,
    profileId,
    engine,
  })
}

// ---------------------------------------------------------------------------
// 4.4 Friction Detection
// ---------------------------------------------------------------------------

export function getFrictionSignals(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    FrictionSignal[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_friction_signals', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.5 Reopened Investigations
// ---------------------------------------------------------------------------

export function getReopenedInvestigations(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    ReopenedInvestigation[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_reopened_investigations', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.6 Browsing Rhythm
// ---------------------------------------------------------------------------

export function getBrowsingRhythm(
  dateRange: DateRange,
  profileId?: string | null,
  category?: string,
) {
  return invokeRequest<
    RhythmHeatmap,
    {
      dateRange: DateRange
      profileId?: string | null
      category?: string
    }
  >('get_browsing_rhythm', {
    dateRange,
    profileId,
    category,
  })
}

// ---------------------------------------------------------------------------
// 4.7 Discovery Trend
// ---------------------------------------------------------------------------

export function getDiscoveryTrend(
  dateRange: DateRange,
  profileId?: string | null,
  granularity?: string,
) {
  return invokeRequest<
    DiscoveryTrend,
    {
      dateRange: DateRange
      profileId?: string | null
      granularity?: string
    }
  >('get_discovery_trend', {
    dateRange,
    profileId,
    granularity,
  })
}

// ---------------------------------------------------------------------------
// 4.8 Activity Mix
// ---------------------------------------------------------------------------

export function getActivityMix(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    ActivityMix,
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_activity_mix', {
    dateRange,
    profileId,
  })
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

// ---------------------------------------------------------------------------
// 4.9 Breadth Index
// ---------------------------------------------------------------------------

export function getBreadthIndex(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    BreadthIndex,
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_breadth_index', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.11 Path Flows
// ---------------------------------------------------------------------------

export function getPathFlows(
  dateRange: DateRange,
  profileId?: string | null,
  stepCount?: number,
  limit?: number,
) {
  return invokeRequest<
    PathFlow[],
    {
      dateRange: DateRange
      profileId?: string | null
      stepCount?: number
      limit?: number
    }
  >('get_path_flows', {
    dateRange,
    profileId,
    stepCount,
    limit,
  })
}

// ---------------------------------------------------------------------------
// 4.12 Compare Sets
// ---------------------------------------------------------------------------

export function getCompareSets(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    CompareSet[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_compare_sets', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.13 Multi-Browser Diff
// ---------------------------------------------------------------------------

export function getMultiBrowserDiff(dateRange: DateRange) {
  return invokeRequest<BrowserDiff, { dateRange: DateRange }>(
    'get_multi_browser_diff',
    { dateRange },
  )
}

// ---------------------------------------------------------------------------
// 4.14 Observed Interactions
// ---------------------------------------------------------------------------

export function getObservedInteractions(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    ObservedInteraction[],
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_observed_interactions', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.A Explainability
// ---------------------------------------------------------------------------

export function explainEntity(entityType: string, entityId: string) {
  return invokeRequest<Explanation, { entityType: string; entityId: string }>(
    'explain_entity',
    {
      entityType,
      entityId,
    },
  )
}
