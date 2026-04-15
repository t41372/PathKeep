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

import { invokeCommand } from '../ipc/bridge'
import type {
  DateRange,
  PaginationParams,
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

// ---------------------------------------------------------------------------
// 1.1 Digest Summary
// ---------------------------------------------------------------------------

export function getDigestSummary(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeCommand<DigestSummary>('get_digest_summary', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 1.2 On This Day
// ---------------------------------------------------------------------------

export function getOnThisDay(profileId?: string | null) {
  return invokeCommand<OnThisDayEntry[]>('get_on_this_day', { profileId })
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
  return invokeCommand<TopSite[]>('get_top_sites', {
    dateRange,
    profileId,
    sortBy,
    limit,
  })
}

export function getDomainTrend(domain: string, dateRange: DateRange) {
  return invokeCommand<DomainTrend>('get_domain_trend', { domain, dateRange })
}

// ---------------------------------------------------------------------------
// 2.2 Search Activity
// ---------------------------------------------------------------------------

export function getSearchEngineRanking(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeCommand<EngineRanking[]>('get_search_engine_ranking', {
    dateRange,
    profileId,
  })
}

export function getTopSearchConcepts(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeCommand<SearchConcept[]>('get_top_search_concepts', {
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
  return invokeCommand<QueryFamilyResult>('get_query_families', {
    dateRange,
    profileId,
    page: pagination?.page ?? 1,
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
  return invokeCommand<RefindPage[]>('get_refind_pages', {
    dateRange,
    profileId,
    limit,
  })
}

export function explainRefind(canonicalUrl: string) {
  return invokeCommand<RefindExplanation>('explain_refind', { canonicalUrl })
}

// ---------------------------------------------------------------------------
// 2.4 Habitual Visits
// ---------------------------------------------------------------------------

export function getHabitPatterns(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeCommand<HabitPattern[]>('get_habit_patterns', {
    dateRange,
    profileId,
  })
}

export function getInterruptedHabits(profileId?: string | null) {
  return invokeCommand<InterruptedHabit[]>('get_interrupted_habits', {
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 3.1 Sessions
// ---------------------------------------------------------------------------

export function getSessions(
  dateRange: DateRange,
  profileId?: string | null,
  pagination?: PaginationParams,
) {
  return invokeCommand<SessionListResult>('get_sessions', {
    dateRange,
    profileId,
    page: pagination?.page ?? 1,
    pageSize: pagination?.pageSize ?? 20,
  })
}

export function getSessionDetail(sessionId: string) {
  return invokeCommand<SessionDetail>('get_session_detail', { sessionId })
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
  return invokeCommand<TrailListResult>('get_search_trails', {
    dateRange,
    profileId,
    engine,
    page: pagination?.page ?? 1,
    pageSize: pagination?.pageSize ?? 20,
  })
}

export function getTrailDetail(trailId: string) {
  return invokeCommand<TrailDetail>('get_trail_detail', { trailId })
}

// ---------------------------------------------------------------------------
// 3.3 Navigation Path
// ---------------------------------------------------------------------------

export function getNavigationPath(visitId: number) {
  return invokeCommand<NavigationPath>('get_navigation_path', { visitId })
}

export function getHubPages(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeCommand<HubPage[]>('get_hub_pages', {
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
  return invokeCommand<DomainDeepDive>('get_domain_deep_dive', {
    domain,
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
  return invokeCommand<StableSource[]>('get_stable_sources', {
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
  return invokeCommand<SearchEffectiveness>('get_search_effectiveness', {
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
  return invokeCommand<FrictionSignal[]>('get_friction_signals', {
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
  return invokeCommand<ReopenedInvestigation[]>('get_reopened_investigations', {
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
  return invokeCommand<RhythmHeatmap>('get_browsing_rhythm', {
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
  return invokeCommand<DiscoveryTrend>('get_discovery_trend', {
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
  return invokeCommand<ActivityMix>('get_activity_mix', {
    dateRange,
    profileId,
  })
}

export function getActivityMixTrend(
  dateRange: DateRange,
  profileId?: string | null,
  granularity?: string,
) {
  return invokeCommand<ActivityMixTrend>('get_activity_mix_trend', {
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
  return invokeCommand<BreadthIndex>('get_breadth_index', {
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
  return invokeCommand<PathFlow[]>('get_path_flows', {
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
  return invokeCommand<CompareSet[]>('get_compare_sets', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.13 Multi-Browser Diff
// ---------------------------------------------------------------------------

export function getMultiBrowserDiff(dateRange: DateRange) {
  return invokeCommand<BrowserDiff>('get_multi_browser_diff', { dateRange })
}

// ---------------------------------------------------------------------------
// 4.14 Observed Interactions
// ---------------------------------------------------------------------------

export function getObservedInteractions(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeCommand<ObservedInteraction[]>('get_observed_interactions', {
    dateRange,
    profileId,
  })
}

// ---------------------------------------------------------------------------
// 4.A Explainability
// ---------------------------------------------------------------------------

export function explainEntity(entityType: string, entityId: string) {
  return invokeCommand<Explanation>('explain_entity', {
    entityType,
    entityId,
  })
}
