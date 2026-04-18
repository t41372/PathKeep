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
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  CoreIntelligenceSectionWindow,
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
  IntelligenceEmbedCardPayload,
  IntelligenceWidgetSnapshot,
  IntelligencePublicSnapshot,
} from './types'

function invokeRequest<TResponse, TRequest extends Record<string, unknown>>(
  command: string,
  request: TRequest,
) {
  return call<TResponse>(command, { request })
}

function directSectionFallback(
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
): CoreIntelligenceSectionMeta {
  return {
    sectionId,
    generatedAt: null,
    window,
    moduleIds: [],
    sourceTables: [],
    includesEnrichment: false,
    state: 'degraded',
    stateReason: null,
    notes: [],
  }
}

function formatLocalDateKey(date: Date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function normalizeSectionResult<T>(
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
  result: CoreIntelligenceSectionResult<T> | T,
): CoreIntelligenceSectionResult<T> {
  if (
    result &&
    typeof result === 'object' &&
    'data' in result &&
    'meta' in result
  ) {
    return result
  }

  return {
    data: result,
    meta: directSectionFallback(sectionId, window),
  }
}

function invokeSectionRequest<
  TResponse,
  TRequest extends Record<string, unknown>,
>(
  command: string,
  request: TRequest,
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
) {
  return call<CoreIntelligenceSectionResult<TResponse> | TResponse>(command, {
    request,
  }).then((result) => normalizeSectionResult(sectionId, window, result))
}

function invokeSectionArgs<TResponse>(
  command: string,
  args: Record<string, unknown>,
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
) {
  return call<CoreIntelligenceSectionResult<TResponse> | TResponse>(
    command,
    args,
  ).then((result) => normalizeSectionResult(sectionId, window, result))
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

// ---------------------------------------------------------------------------
// 1.2 On This Day
// ---------------------------------------------------------------------------

export function getOnThisDay(profileId?: string | null) {
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

// ---------------------------------------------------------------------------
// 2.1 Top Sites
// ---------------------------------------------------------------------------

export function getTopSites(
  dateRange: DateRange,
  profileId?: string | null,
  sortBy?: string,
  limit?: number,
) {
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

export function getQueryFamilies(
  dateRange: DateRange,
  profileId?: string | null,
  pagination?: PaginationParams,
) {
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

// ---------------------------------------------------------------------------
// 2.3 Refind Pages
// ---------------------------------------------------------------------------

export function getRefindPages(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
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
  return invokeSectionRequest<
    DomainDeepDive,
    {
      registrableDomain: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_domain_deep_dive',
    {
      registrableDomain: domain,
      dateRange,
      profileId,
    },
    'domain-deep-dive',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

// ---------------------------------------------------------------------------
// 4.2 Stable Sources
// ---------------------------------------------------------------------------

export function getStableSources(
  dateRange: DateRange,
  profileId?: string | null,
) {
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

// ---------------------------------------------------------------------------
// 4.3 Search Effectiveness
// ---------------------------------------------------------------------------

export function getSearchEffectiveness(
  dateRange: DateRange,
  profileId?: string | null,
  engine?: string,
) {
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

// ---------------------------------------------------------------------------
// 4.4 Friction Detection
// ---------------------------------------------------------------------------

export function getFrictionSignals(
  dateRange: DateRange,
  profileId?: string | null,
) {
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

// ---------------------------------------------------------------------------
// 4.5 Reopened Investigations
// ---------------------------------------------------------------------------

export function getReopenedInvestigations(
  dateRange: DateRange,
  profileId?: string | null,
) {
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

// ---------------------------------------------------------------------------
// 4.6 Browsing Rhythm
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4.7 Discovery Trend
// ---------------------------------------------------------------------------

export function getDiscoveryTrend(
  dateRange: DateRange,
  profileId?: string | null,
  granularity?: string,
) {
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

// ---------------------------------------------------------------------------
// 4.8 Activity Mix
// ---------------------------------------------------------------------------

export function getActivityMix(
  dateRange: DateRange,
  profileId?: string | null,
) {
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

// ---------------------------------------------------------------------------
// 4.9 Breadth Index
// ---------------------------------------------------------------------------

export function getBreadthIndex(
  dateRange: DateRange,
  profileId?: string | null,
) {
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

// ---------------------------------------------------------------------------
// 4.11 Path Flows
// ---------------------------------------------------------------------------

export function getPathFlows(
  dateRange: DateRange,
  profileId?: string | null,
  stepCount?: number,
  limit?: number,
) {
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

// ---------------------------------------------------------------------------
// 4.12 Compare Sets
// ---------------------------------------------------------------------------

export function getCompareSets(
  dateRange: DateRange,
  profileId?: string | null,
) {
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

// ---------------------------------------------------------------------------
// 4.13 Multi-Browser Diff
// ---------------------------------------------------------------------------

export function getMultiBrowserDiff(dateRange: DateRange) {
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

// ---------------------------------------------------------------------------
// 4.14 Observed Interactions
// ---------------------------------------------------------------------------

export function getObservedInteractions(
  dateRange: DateRange,
  profileId?: string | null,
) {
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

// ---------------------------------------------------------------------------
// 4.B External Output Payload Providers
// ---------------------------------------------------------------------------

export function getIntelligenceEmbedCards(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    IntelligenceEmbedCardPayload[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_intelligence_embed_cards', {
    dateRange,
    profileId,
    limit,
  })
}

export function getIntelligenceWidgetSnapshot(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    IntelligenceWidgetSnapshot,
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_intelligence_widget_snapshot', {
    dateRange,
    profileId,
    limit,
  })
}

export function getIntelligencePublicSnapshot(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    IntelligencePublicSnapshot,
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_intelligence_public_snapshot', {
    dateRange,
    profileId,
  })
}
