/**
 * Read commands for the deterministic insights (computed locally from the
 * archive by the core-intelligence jobs, no AI involved).
 *
 * Each section comes back as `{ data, meta }`. `meta.state` is `stale` while
 * the background job that refreshes it is still running after a backup, so
 * callers can show the old numbers and poll until they settle.
 */
import type {
  ActivityMix,
  ActivityMixTrend,
  BreadthIndex,
  BrowserDiff,
  CompareSet,
  CoreIntelligenceSectionMeta,
  DateRange,
  DayInsights,
  DigestSummary,
  DiscoveryTrend,
  DomainDeepDive,
  EngineRanking,
  FrequentSearch,
  FrictionSignal,
  HabitPattern,
  InterruptedHabit,
  ObservedInteraction,
  OnThisDayEntry,
  Page,
  PathFlow,
  QueryFamilyDetail,
  QueryFamilyResult,
  RefindPage,
  RefindPageDetail,
  ReopenedInvestigation,
  RhythmHeatmap,
  SearchConcept,
  SearchEffectiveness,
  StableSource,
  SearchQueryListResult,
  SearchQuerySort,
  SessionDetail,
  SessionListResult,
  TopSite,
  TrailDetail,
} from '../core-intelligence/types'
import { call } from './shared'

export interface Section<T> {
  data: T
  meta: CoreIntelligenceSectionMeta | null
}

async function section<T>(command: string, args: Record<string, unknown>) {
  const result = await call<Section<T> | T>(command, args)
  if (
    result &&
    typeof result === 'object' &&
    'data' in result &&
    'meta' in result
  ) {
    return result
  }
  return { data: result as T, meta: null }
}

interface Scope {
  dateRange: DateRange
  profileId?: string | null
  /** Several profiles summed together; only the reads that say so accept it. */
  profileIds?: string[]
}

export const insightsClient = {
  digest: (scope: Scope) =>
    section<DigestSummary>('get_digest_summary', { request: scope }),
  /** Per day; charts group days into weeks themselves (see insights/buckets). */
  discoveryTrend: (scope: Scope) =>
    section<DiscoveryTrend>('get_discovery_trend', {
      request: { ...scope, granularity: 'day' },
    }),
  /** Each category's share of the scope's visits, and its change from the period before. */
  activityMix: (scope: Scope) =>
    section<ActivityMix>('get_activity_mix', { request: scope }),
  /** Per day; charts group days into weeks themselves (see insights/buckets). */
  activityMixTrend: (scope: Scope) =>
    call<ActivityMixTrend>('get_activity_mix_trend', {
      request: { ...scope, granularity: 'day' },
    }),
  onThisDay: (profileId?: string | null) =>
    section<OnThisDayEntry[]>('get_on_this_day', { profileId }),
  /** One page of sites; `limit` is the page size. */
  topSites: (
    scope: Scope,
    limit = 10,
    cursor: string | null = null,
    sortBy: 'visits' | 'unique_days' | 'average_daily_visits' = 'visits',
  ) =>
    section<Page<TopSite>>('get_top_sites', {
      request: { ...scope, sortBy, limit, cursor },
    }),
  rhythm: (scope: Scope) =>
    section<RhythmHeatmap>('get_browsing_rhythm', { request: scope }),
  searchConcepts: (scope: Scope, limit = 12, cursor: string | null = null) =>
    section<Page<SearchConcept>>('get_top_search_concepts', {
      request: { ...scope, limit, cursor },
    }),
  /** Most searched queries, counted inside the range and merged across engines. */
  frequentSearches: (scope: Scope, limit = 10, cursor: string | null = null) =>
    section<Page<FrequentSearch>>('get_frequent_searches', {
      request: { ...scope, limit, cursor },
    }),
  /** Every family holding `query`, across all time and browsers. */
  searchFamilies: (query: string, profileId: string | null = null) =>
    section<QueryFamilyResult>('get_search_families', {
      request: { query, profileId },
    }),
  queryFamilies: (scope: Scope, pageSize = 200, cursor: string | null = null) =>
    section<QueryFamilyResult>('get_query_families', {
      request: { ...scope, cursor, pageSize },
    }),
  refindPages: (scope: Scope, limit = 8, cursor: string | null = null) =>
    section<Page<RefindPage>>('get_refind_pages', {
      request: { ...scope, limit, cursor },
    }),
  reopenedInvestigations: (
    scope: Scope,
    limit = 20,
    cursor: string | null = null,
  ) =>
    section<Page<ReopenedInvestigation>>('get_reopened_investigations', {
      request: { ...scope, limit, cursor },
    }),

  // Drill-ins. Detail reads are bounded by one entity; see
  // docs/architecture/desktop-command-surface.md for what each one reads.

  /** One local calendar day: KPIs against the day before, hours, top sites. */
  day: (date: string) =>
    section<DayInsights>('get_day_insights', { request: { date } }),
  /** Browsing sessions that overlap the range, newest first. */
  sessions: (scope: Scope, pageSize = 50, cursor: string | null = null) =>
    call<SessionListResult>('get_sessions', {
      request: { ...scope, cursor, pageSize },
    }),
  sessionDetail: (sessionId: string) =>
    call<SessionDetail>('get_session_detail', { sessionId }),
  /** Searches made in the range, one row per distinct query and engine. */
  searchQueries: (
    scope: Scope,
    options: {
      sort?: SearchQuerySort
      pageSize?: number
      cursor?: string | null
    } = {},
  ) =>
    section<SearchQueryListResult>('get_search_queries', {
      request: {
        ...scope,
        sort: options.sort ?? 'newest',
        cursor: options.cursor ?? null,
        pageSize: options.pageSize ?? 50,
      },
    }),
  site: (domain: string, scope: Scope) =>
    section<DomainDeepDive>('get_domain_deep_dive', {
      request: { ...scope, registrableDomain: domain },
    }),
  queryFamilyDetail: (familyId: string, scope: Scope) =>
    section<QueryFamilyDetail>('get_query_family_detail', {
      request: { ...scope, familyId },
    }),
  trailDetail: (trailId: string) =>
    call<TrailDetail>('get_trail_detail', { trailId }),
  refindPageDetail: (canonicalUrl: string, scope: Scope) =>
    section<RefindPageDetail>('get_refind_page_detail', {
      request: { ...scope, canonicalUrl },
    }),

  // Patterns on the Insights screen.

  breadth: (scope: Scope) =>
    section<BreadthIndex>('get_breadth_index', { request: scope }),
  habits: (scope: Scope) =>
    section<HabitPattern[]>('get_habit_patterns', { request: scope }),
  /** Per browser profile; ignores `profileId`. */
  browsers: (scope: Scope) =>
    section<BrowserDiff>('get_multi_browser_diff', { request: scope }),
  /** Habits whose usual gap has passed twice over, as of the last Insights build. */
  interruptedHabits: (profileId: string | null = null) =>
    section<InterruptedHabit[]>('get_interrupted_habits', {
      request: { profileId },
    }),
  searchEngines: (scope: Scope) =>
    section<EngineRanking[]>('get_search_engine_ranking', { request: scope }),
  /** Sequences of sites visited one after another, most frequent first. */
  paths: (scope: Scope, steps = 3, limit = 15, cursor: string | null = null) =>
    section<Page<PathFlow>>('get_path_flows', {
      request: { ...scope, stepCount: steps, limit, cursor },
    }),
  /** Per engine: searches, rewordings and pages opened; and the searches come back to most. */
  searchEffectiveness: (scope: Scope) =>
    section<SearchEffectiveness>('get_search_effectiveness', {
      request: {
        ...scope,
        engine: null,
        topicsLimit: null,
        topicsCursor: null,
      },
    }),
  /** The sites searches ended on, most first. */
  searchLandings: (scope: Scope, limit = 10, cursor: string | null = null) =>
    section<Page<StableSource>>('get_stable_sources', {
      request: { ...scope, limit, cursor },
    }),
  /** Searches reworded twice or more, or that never settled on a page. */
  friction: (scope: Scope, limit = 10, cursor: string | null = null) =>
    section<Page<FrictionSignal>>('get_friction_signals', {
      request: { ...scope, limit, cursor },
    }),
  /** Pages of one kind gone back and forth between within one search, newest first. */
  compareSets: (scope: Scope, limit = 6, cursor: string | null = null) =>
    section<Page<CompareSet>>('get_compare_sets', {
      request: { ...scope, limit, cursor },
    }),
  /** What Chromium recorded about time on a page, newest first. */
  observedInteractions: (
    scope: Scope,
    limit = 10,
    cursor: string | null = null,
  ) =>
    section<Page<ObservedInteraction>>('get_observed_interactions', {
      request: { ...scope, limit, cursor },
    }),
}

/** True while the background job that refreshes this section is running. */
export function isStale(
  result: { meta: { state: string } | null } | undefined,
) {
  return result?.meta?.state === 'stale'
}

/** Keeps polling a section while its background refresh is still running. */
export function pollWhileStale(query: {
  state: { data?: { meta: { state: string } | null } }
}): number | false {
  return isStale(query.state.data) ? 5_000 : false
}

/** `YYYY-MM-DD` in local time, the format every insights command expects. */
export function localDateKey(date: Date) {
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** The last `days` days, ending today. */
export function lastDays(days: number): DateRange {
  const end = new Date()
  const start = new Date(end)
  start.setDate(start.getDate() - (days - 1))
  return { start: localDateKey(start), end: localDateKey(end) }
}

export function calendarYear(year: number): DateRange {
  return { start: `${year}-01-01`, end: `${year}-12-31` }
}
