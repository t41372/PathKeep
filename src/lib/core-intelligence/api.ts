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
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceSecondaryOverview,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  CoreIntelligenceSectionTiming,
  CoreIntelligenceSectionWindow,
  DayInsights,
  DigestSummary,
  OnThisDayEntry,
  TopSite,
  DomainTrend,
  EngineRanking,
  SearchConcept,
  SearchQueryListResult,
  SearchQuerySort,
  QueryFamilyDetail,
  QueryFamilyResult,
  RefindPageDetail,
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
  CompareSetDetail,
  BrowserDiff,
  ObservedInteraction,
  Explanation,
  IntelligenceEmbedCardPayload,
  IntelligenceLocalHostBuildResult,
  IntelligenceLocalHostPreview,
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

function normalizeSectionWindow(
  window: unknown,
  fallback: CoreIntelligenceSectionWindow,
): CoreIntelligenceSectionWindow {
  if (!window || typeof window !== 'object') {
    return fallback
  }

  const raw = window as Record<string, unknown>
  if (raw.kind === 'date-range') {
    const rawDateRange =
      raw.dateRange && typeof raw.dateRange === 'object'
        ? raw.dateRange
        : raw.date_range && typeof raw.date_range === 'object'
          ? raw.date_range
          : null
    const start =
      rawDateRange &&
      typeof (rawDateRange as Record<string, unknown>).start === 'string'
        ? ((rawDateRange as Record<string, unknown>).start as string)
        : fallback.kind === 'date-range'
          ? fallback.dateRange.start
          : ''
    const end =
      rawDateRange &&
      typeof (rawDateRange as Record<string, unknown>).end === 'string'
        ? ((rawDateRange as Record<string, unknown>).end as string)
        : fallback.kind === 'date-range'
          ? fallback.dateRange.end
          : ''

    return {
      kind: 'date-range',
      dateRange: { start, end },
    }
  }

  if (raw.kind === 'calendar-day-history') {
    const referenceDate =
      typeof raw.referenceDate === 'string'
        ? raw.referenceDate
        : typeof raw.reference_date === 'string'
          ? raw.reference_date
          : fallback.kind === 'calendar-day-history'
            ? fallback.referenceDate
            : formatLocalDateKey(new Date())

    return {
      kind: 'calendar-day-history',
      referenceDate,
    }
  }

  return fallback
}

function normalizeSectionMeta(
  sectionId: string,
  fallbackWindow: CoreIntelligenceSectionWindow,
  meta: unknown,
): CoreIntelligenceSectionMeta {
  if (!meta || typeof meta !== 'object') {
    return directSectionFallback(sectionId, fallbackWindow)
  }

  const raw = meta as Record<string, unknown>
  const state =
    raw.state === 'ready' ||
    raw.state === 'stale' ||
    raw.state === 'disabled' ||
    raw.state === 'degraded'
      ? raw.state
      : 'degraded'

  return {
    sectionId: typeof raw.sectionId === 'string' ? raw.sectionId : sectionId,
    generatedAt:
      typeof raw.generatedAt === 'string' || raw.generatedAt === null
        ? (raw.generatedAt ?? null)
        : null,
    window: normalizeSectionWindow(raw.window, fallbackWindow),
    moduleIds: Array.isArray(raw.moduleIds)
      ? raw.moduleIds.filter(
          (value): value is string => typeof value === 'string',
        )
      : [],
    sourceTables: Array.isArray(raw.sourceTables)
      ? raw.sourceTables.filter(
          (value): value is string => typeof value === 'string',
        )
      : [],
    includesEnrichment: Boolean(raw.includesEnrichment),
    state,
    stateReason:
      typeof raw.stateReason === 'string' || raw.stateReason === null
        ? (raw.stateReason ?? null)
        : null,
    notes: Array.isArray(raw.notes)
      ? raw.notes.filter((value): value is string => typeof value === 'string')
      : [],
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
    return {
      data: result.data,
      meta: normalizeSectionMeta(sectionId, window, result.meta),
    }
  }

  return {
    data: result,
    meta: directSectionFallback(sectionId, window),
  }
}

interface OverviewCacheEntry {
  primary?: CoreIntelligencePrimaryOverview
  secondary?: CoreIntelligenceSecondaryOverview
}

const overviewCache = new Map<string, OverviewCacheEntry>()

function overviewScopeKey(dateRange: DateRange, profileId?: string | null) {
  return JSON.stringify({
    dateRange,
    profileId: profileId ?? null,
  })
}

function readOverviewCache(dateRange: DateRange, profileId?: string | null) {
  return overviewCache.get(overviewScopeKey(dateRange, profileId)) ?? null
}

function writeOverviewCache(
  dateRange: DateRange,
  profileId: string | null | undefined,
  updater: (current: OverviewCacheEntry) => OverviewCacheEntry,
) {
  const key = overviewScopeKey(dateRange, profileId)
  const current = overviewCache.get(key) ?? {}
  overviewCache.set(key, updater(current))
}

function cachedPrimaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return readOverviewCache(dateRange, profileId)?.primary ?? null
}

function cachedSecondaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return readOverviewCache(dateRange, profileId)?.secondary ?? null
}

function cachedPrimarySectionForProfile<T>(
  profileId: string | null | undefined,
  read: (
    overview: CoreIntelligencePrimaryOverview,
  ) => CoreIntelligenceSectionResult<T> | undefined,
) {
  const targetProfile = profileId ?? null
  for (const [key, entry] of overviewCache.entries()) {
    const parsed = JSON.parse(key) as {
      profileId?: string | null
    }
    if ((parsed.profileId ?? null) !== targetProfile) {
      continue
    }
    const section = entry.primary ? read(entry.primary) : undefined
    if (section) {
      return section
    }
  }

  return null
}

function cachedSecondarySectionForDateRange<T>(
  dateRange: DateRange,
  read: (
    overview: CoreIntelligenceSecondaryOverview,
  ) => CoreIntelligenceSectionResult<T> | undefined,
) {
  for (const [key, entry] of overviewCache.entries()) {
    const parsed = JSON.parse(key) as {
      dateRange?: DateRange
    }
    if (
      parsed.dateRange?.start !== dateRange.start ||
      parsed.dateRange?.end !== dateRange.end
    ) {
      continue
    }
    const section = entry.secondary ? read(entry.secondary) : undefined
    if (section) {
      return section
    }
  }

  return null
}

function normalizeTiming(
  timing: unknown,
): CoreIntelligenceSectionTiming | null {
  if (!timing || typeof timing !== 'object') {
    return null
  }

  const raw = timing as Record<string, unknown>
  if (typeof raw.sectionId !== 'string' || typeof raw.durationMs !== 'number') {
    return null
  }

  return {
    sectionId: raw.sectionId,
    durationMs: raw.durationMs,
  }
}

function normalizeTimings(value: unknown) {
  if (!Array.isArray(value)) return []
  return value
    .map(normalizeTiming)
    .filter(
      (timing): timing is CoreIntelligenceSectionTiming => timing !== null,
    )
}

function normalizePrimaryOverview(
  dateRange: DateRange,
  result: CoreIntelligencePrimaryOverview,
): CoreIntelligencePrimaryOverview {
  return {
    digestSummary: normalizeSectionResult(
      'digest-summary',
      { kind: 'date-range', dateRange },
      result.digestSummary,
    ),
    onThisDay: normalizeSectionResult(
      'on-this-day',
      {
        kind: 'calendar-day-history',
        referenceDate: formatLocalDateKey(new Date()),
      },
      result.onThisDay,
    ),
    topSites: normalizeSectionResult(
      'top-sites',
      { kind: 'date-range', dateRange },
      result.topSites,
    ),
    refindPages: normalizeSectionResult(
      'refind-pages',
      { kind: 'date-range', dateRange },
      result.refindPages,
    ),
    searchEngineRanking: normalizeSectionResult(
      'search-activity',
      { kind: 'date-range', dateRange },
      result.searchEngineRanking,
    ),
    topSearchConcepts: normalizeSectionResult(
      'search-activity',
      { kind: 'date-range', dateRange },
      result.topSearchConcepts,
    ),
    queryFamilies: normalizeSectionResult(
      'search-activity',
      { kind: 'date-range', dateRange },
      result.queryFamilies,
    ),
    activityMix: normalizeSectionResult(
      'activity-mix',
      { kind: 'date-range', dateRange },
      result.activityMix,
    ),
    discoveryTrendDay: normalizeSectionResult(
      'browsing-rhythm',
      { kind: 'date-range', dateRange },
      result.discoveryTrendDay,
    ),
    habitPatterns: normalizeSectionResult(
      'habits',
      { kind: 'date-range', dateRange },
      result.habitPatterns,
    ),
    interruptedHabits: normalizeSectionResult(
      'habits',
      { kind: 'date-range', dateRange },
      result.interruptedHabits,
    ),
    timings: normalizeTimings(result.timings),
    totalDurationMs:
      typeof result.totalDurationMs === 'number' ? result.totalDurationMs : 0,
  }
}

function normalizeSecondaryOverview(
  dateRange: DateRange,
  result: CoreIntelligenceSecondaryOverview,
): CoreIntelligenceSecondaryOverview {
  return {
    stableSources: normalizeSectionResult(
      'stable-sources',
      { kind: 'date-range', dateRange },
      result.stableSources,
    ),
    searchEffectiveness: normalizeSectionResult(
      'search-effectiveness',
      { kind: 'date-range', dateRange },
      result.searchEffectiveness,
    ),
    frictionSignals: normalizeSectionResult(
      'friction-signals',
      { kind: 'date-range', dateRange },
      result.frictionSignals,
    ),
    reopenedInvestigations: normalizeSectionResult(
      'reopened-investigations',
      { kind: 'date-range', dateRange },
      result.reopenedInvestigations,
    ),
    discoveryTrendWeek: normalizeSectionResult(
      'discovery-trend',
      { kind: 'date-range', dateRange },
      result.discoveryTrendWeek,
    ),
    breadthIndex: normalizeSectionResult(
      'breadth-index',
      { kind: 'date-range', dateRange },
      result.breadthIndex,
    ),
    pathFlows: normalizeSectionResult(
      'path-flows',
      { kind: 'date-range', dateRange },
      result.pathFlows,
    ),
    compareSets: normalizeSectionResult(
      'compare-sets',
      { kind: 'date-range', dateRange },
      result.compareSets,
    ),
    multiBrowserDiff: normalizeSectionResult(
      'multi-browser-diff',
      { kind: 'date-range', dateRange },
      result.multiBrowserDiff,
    ),
    observedInteractions: normalizeSectionResult(
      'observed-interactions',
      { kind: 'date-range', dateRange },
      result.observedInteractions,
    ),
    timings: normalizeTimings(result.timings),
    totalDurationMs:
      typeof result.totalDurationMs === 'number' ? result.totalDurationMs : 0,
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
  overviewCache.clear()
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

// ---------------------------------------------------------------------------
// 1.1 Digest Summary
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 1.2 On This Day
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 2.1 Top Sites
// ---------------------------------------------------------------------------

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
    pagination?: PaginationParams
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
  pagination?: PaginationParams,
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

export function getQueryFamilyDetail(
  familyId: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeSectionRequest<
    QueryFamilyDetail,
    {
      familyId: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_query_family_detail',
    {
      familyId,
      dateRange,
      profileId,
    },
    'query-family-detail',
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

export function getRefindPageDetail(
  canonicalUrl: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeSectionRequest<
    RefindPageDetail,
    {
      canonicalUrl: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_refind_page_detail',
    {
      canonicalUrl,
      dateRange,
      profileId,
    },
    'refind-page-detail',
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

export function getDayInsights(date: string, profileId?: string | null) {
  return invokeSectionRequest<
    DayInsights,
    {
      date: string
      profileId?: string | null
    }
  >(
    'get_day_insights',
    {
      date,
      profileId,
    },
    'day-insights',
    {
      kind: 'date-range',
      dateRange: {
        start: date,
        end: date,
      },
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

// ---------------------------------------------------------------------------
// 4.3 Search Effectiveness
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4.4 Friction Detection
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4.5 Reopened Investigations
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4.8 Activity Mix
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4.9 Breadth Index
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4.11 Path Flows
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4.12 Compare Sets
// ---------------------------------------------------------------------------

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

export function getCompareSetDetail(
  compareSetId: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeSectionRequest<
    CompareSetDetail,
    {
      compareSetId: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_compare_set_detail',
    {
      compareSetId,
      dateRange,
      profileId,
    },
    'compare-set-detail',
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

// ---------------------------------------------------------------------------
// 4.14 Observed Interactions
// ---------------------------------------------------------------------------

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

export function previewIntelligenceLocalHost(
  dateRange: DateRange,
  locale: string,
  profileId?: string | null,
) {
  return invokeRequest<
    IntelligenceLocalHostPreview,
    {
      dateRange: DateRange
      profileId?: string | null
      locale: string
    }
  >('preview_intelligence_local_host', {
    dateRange,
    profileId,
    locale,
  })
}

export function buildIntelligenceLocalHost(
  dateRange: DateRange,
  locale: string,
  profileId?: string | null,
) {
  return invokeRequest<
    IntelligenceLocalHostBuildResult,
    {
      dateRange: DateRange
      profileId?: string | null
      locale: string
    }
  >('build_intelligence_local_host', {
    dateRange,
    profileId,
    locale,
  })
}
