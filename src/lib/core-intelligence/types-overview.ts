/**
 * @file types-overview.ts
 * @description Primary and secondary overview payloads plus the deterministic search/refind/habit entities shown across dashboard and intelligence surfaces.
 * @module core-intelligence/types
 *
 * ## Responsibilities
 * - Own overview read models and the deterministic entities that populate first-band and secondary intelligence cards.
 * - Keep search, refind, habit, and exact-day payload contracts grouped together for route reuse.
 *
 * ## Not responsible for
 * - Owning session/trail navigation payloads.
 * - Owning domain-deep-dive, friction, compare-set, or multi-browser analysis contracts.
 *
 * ## Dependencies
 * - Depends on shared section envelope primitives plus navigation and analysis detail types.
 * - Consumed by dashboard, intelligence overview loaders, and settings derived-state surfaces.
 *
 * ## Performance notes
 * - Type-only module; grouping overview-facing entities together keeps staged overview loader contracts easy to audit.
 */

import type {
  CoreIntelligenceSectionResult,
  CoreIntelligenceSectionTiming,
  DateRange,
  KpiMetric,
  Page,
} from './types-primitives'
import type { TrailSummary } from './types-navigation'
import type {
  ActivityMix,
  BreadthIndex,
  BrowserDiff,
  CompareSet,
  DiscoveryTrend,
  FrictionSignal,
  ObservedInteraction,
  PathFlow,
  ReopenedInvestigation,
  SearchEffectiveness,
  StableSource,
} from './types-analysis'

// ---------------------------------------------------------------------------
// 1.1 Digest Summary (時間段摘要)
// ---------------------------------------------------------------------------

/** Response from `get_digest_summary` command. */
export interface DigestSummary {
  dateRange: DateRange
  totalVisits: KpiMetric
  totalSearches: KpiMetric
  newDomains: KpiMetric
  deepReadPages: KpiMetric
  refindPages: KpiMetric
  /** Distinct registrable domains visited in the range. */
  distinctDomains: KpiMetric
  /** Estimated time spent actively browsing, in milliseconds. */
  activeTimeMs: KpiMetric
}

/** First-band `/intelligence` payload used to keep route entry responsive. */
export interface CoreIntelligencePrimaryOverview {
  digestSummary: CoreIntelligenceSectionResult<DigestSummary>
  onThisDay: CoreIntelligenceSectionResult<OnThisDayEntry[]>
  topSites: CoreIntelligenceSectionResult<Page<TopSite>>
  refindPages: CoreIntelligenceSectionResult<Page<RefindPage>>
  searchEngineRanking: CoreIntelligenceSectionResult<EngineRanking[]>
  topSearchConcepts: CoreIntelligenceSectionResult<Page<SearchConcept>>
  queryFamilies: CoreIntelligenceSectionResult<QueryFamilyResult>
  activityMix: CoreIntelligenceSectionResult<ActivityMix>
  discoveryTrendDay: CoreIntelligenceSectionResult<DiscoveryTrend>
  habitPatterns: CoreIntelligenceSectionResult<HabitPattern[]>
  interruptedHabits: CoreIntelligenceSectionResult<InterruptedHabit[]>
  timings: CoreIntelligenceSectionTiming[]
  totalDurationMs: number
}

/** Deferred `/intelligence` payload for below-the-fold sections. */
export interface CoreIntelligenceSecondaryOverview {
  stableSources: CoreIntelligenceSectionResult<Page<StableSource>>
  searchEffectiveness: CoreIntelligenceSectionResult<SearchEffectiveness>
  frictionSignals: CoreIntelligenceSectionResult<Page<FrictionSignal>>
  reopenedInvestigations: CoreIntelligenceSectionResult<
    Page<ReopenedInvestigation>
  >
  discoveryTrendWeek: CoreIntelligenceSectionResult<DiscoveryTrend>
  breadthIndex: CoreIntelligenceSectionResult<BreadthIndex>
  pathFlows: CoreIntelligenceSectionResult<Page<PathFlow>>
  compareSets: CoreIntelligenceSectionResult<Page<CompareSet>>
  multiBrowserDiff: CoreIntelligenceSectionResult<BrowserDiff>
  observedInteractions: CoreIntelligenceSectionResult<Page<ObservedInteraction>>
  timings: CoreIntelligenceSectionTiming[]
  totalDurationMs: number
}

export interface DayInsightsDrilldown {
  explorerDateRange: DateRange
}

export interface DayInsightsHourlyBucket {
  hour: number
  visitCount: number
}

/** Exact local-calendar-day deterministic insights surface. */
export interface DayInsights {
  date: string
  digestSummary: DigestSummary
  topSites: Page<TopSite>
  activityMix: ActivityMix
  refindPages: Page<RefindPage>
  queryFamilies: QueryFamilyResult
  hourlyActivity: DayInsightsHourlyBucket[]
  drilldown: DayInsightsDrilldown
}

// ---------------------------------------------------------------------------
// 1.2 On This Day (歷史上的今天)
// ---------------------------------------------------------------------------

/** A single "on this day" entry for a past year. */
export interface OnThisDayEntry {
  year: number
  date: string
  totalVisits: number
  topDomains: string[]
  /** More of that day's sites, through the top-sites read for that date. */
  topDomainsNextCursor: string | null
  /** Human-readable summary, e.g. "在研究 Tauri v2 文檔" */
  summary?: string | null
  /** Deep dive sessions on that day */
  deepDiveSessions: number
}

// ---------------------------------------------------------------------------
// 2.1 Top Sites (Top 網站統計)
// ---------------------------------------------------------------------------

export interface TopSite {
  registrableDomain: string
  displayName?: string | null
  domainCategory: string
  visitCount: number
  uniqueDays: number
  averageDailyVisits: number
  uniqueUrls: number
}

export interface DomainTrendPoint {
  dateKey: string
  visitCount: number
}

export interface DomainTrend {
  registrableDomain: string
  points: DomainTrendPoint[]
}

// ---------------------------------------------------------------------------
// 2.2 Search Activity (搜索活動)
// ---------------------------------------------------------------------------

/** Engine ranking entry */
export interface EngineRanking {
  searchEngine: string
  displayName?: string | null
  searchCount: number
  /** The site its searches are on, for its icon. */
  site?: string | null
}

/**
 * One query and how often it was searched inside the requested date range
 * (`get_frequent_searches`). Counts are range-scoped, unlike query families.
 */
export interface FrequentSearch {
  /** The most recent spelling the user typed. */
  query: string
  /** Lower-cased, whitespace-collapsed grouping key. */
  normalizedQuery: string
  /** Keyword searches inside the range, across every search engine. */
  searchCount: number
  lastSearchedAt: string
}

/** Token frequency for word cloud */
export interface SearchConcept {
  term: string
  frequency: number
  /** Which engines contributed to this term */
  engines: string[]
}

export interface SearchQueryRow {
  visitId: number
  profileId: string
  browserKind: string
  searchEngine: string
  displayName?: string | null
  rawQuery: string
  normalizedQuery: string
  searchedAt: string
  searchedAtMs: number
  exactRepeatCount: number
  familyCount: number
  familyId?: string | null
  trailId?: string | null
  trailInitialQuery?: string | null
  trailReformulationCount?: number | null
}

export interface SearchQueryListResult {
  rows: SearchQueryRow[]
  total: number
  nextCursor: string | null
  pageSize: number
}

export type SearchQuerySort =
  | 'newest'
  | 'exact-frequency'
  | 'family-frequency'
  | 'alphabetical'

export interface SearchEngineRule {
  ruleId: string
  engineId: string
  displayName: string
  hostPattern: string
  pathPrefix?: string | null
  queryParamKey: string
  enabled: boolean
  note?: string | null
  exampleUrl?: string | null
  builtIn: boolean
}

export interface SearchEngineRuleInput {
  ruleId?: string | null
  engineId: string
  displayName: string
  hostPattern: string
  pathPrefix?: string | null
  queryParamKey: string
  enabled: boolean
  note?: string | null
  exampleUrl?: string | null
}

/** A family of related queries */
export interface QueryFamily {
  familyId: string
  anchorQuery: string
  memberCount: number
  searchEngine: string
  queries: string[]
  firstSeenAt: string
  lastSeenAt: string
}

export interface QueryFamilyResult {
  families: QueryFamily[]
  total: number
  nextCursor: string | null
  pageSize: number
}

export interface QueryFamilyDetail {
  family: QueryFamily
  relatedTrails: Page<TrailSummary>
}

// ---------------------------------------------------------------------------
// 2.3 Refind Pages (常重找的頁面)
// ---------------------------------------------------------------------------

export interface RefindPage {
  /** The browser profile this row was scored in; scores are per profile. */
  profileId: string
  /** How many profiles re-found the page. Archive-wide lists keep one row per page. */
  profileCount: number
  canonicalUrl: string
  url: string
  title?: string | null
  registrableDomain: string
  crossDayCount: number
  trailCount: number
  searchArrivalCount: number
  typedRevisitCount: number
  refindScore: number
  firstSeenAt: string
  lastSeenAt: string
}

/** Detailed explanation of why a page is considered a refind page */
export interface RefindExplanation {
  canonicalUrl: string
  refindScore: number
  factors: RefindScoreFactor[]
  visitIds: number[]
  /** The profile this explanation was scored in. */
  profileId: string
  visitIdsNextCursor: string | null
}

export interface RefindPageDetail {
  page: RefindPage
  explanation: RefindExplanation
  recentDays: string[]
  relatedTrails: Page<TrailSummary>
}

export interface RefindScoreFactor {
  signal: string
  rawValue: number
  weight: number
  contribution: number
}

// ---------------------------------------------------------------------------
// 2.4 Habitual Visit Detector (定期訪問偵測)
// ---------------------------------------------------------------------------

export type HabitType = 'daily_habit' | 'weekly_habit' | 'periodic_reference'

export interface HabitPattern {
  registrableDomain: string
  displayName?: string | null
  habitType: HabitType
  meanIntervalDays: number
  cv: number
  visitCount: number
  lastVisitedAt: string
  isInterrupted: boolean
}

export interface InterruptedHabit extends HabitPattern {
  /** Days since last visit */
  daysSinceLastVisit: number
  /** Expected interval * 2 */
  interruptionThresholdDays: number
}
