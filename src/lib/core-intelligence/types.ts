/**
 * Typed front-end contracts for Core Intelligence (non-LLM, deterministic analysis).
 *
 * Why this file exists:
 * - Core Intelligence is the deterministic analysis layer that works without LLM/embedding.
 * - These types define the IPC contract between the Rust backend and the React frontend.
 * - Separated from `src/lib/types/intelligence.ts` which covers AI/LLM types.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md` §5 (Derived Tables Schema)
 * - Tauri command surface defined in the implementation plan
 */

// ---------------------------------------------------------------------------
// Shared primitives
// ---------------------------------------------------------------------------

/** ISO date range used by most Core Intelligence queries. */
export interface DateRange {
  /** ISO 8601 date string, e.g. '2025-01-01' */
  start: string
  /** ISO 8601 date string, e.g. '2025-12-31' */
  end: string
}

/** Standard pagination params. */
export interface PaginationParams {
  page: number
  pageSize: number
}

/** Request payload for a scoped or full Core Intelligence rebuild. */
export interface CoreIntelligenceRebuildRequest {
  profileId?: string | null
  fullRebuild: boolean
  limit?: number | null
}

/** Result payload returned after a Core Intelligence rebuild finishes. */
export interface CoreIntelligenceRebuildReport {
  runId: number
  processedVisits: number
  visitDerivedFacts: number
  sessions: number
  searchTrails: number
  queryFamilies: number
  refindPages: number
  sourceEffectiveness: number
  reopenedInvestigations: number
  stageTimingsMs?: CoreIntelligenceStageTimings | null
  notes: string[]
  lastRunAt: string
}

/** Queue acknowledgement for a manual Core Intelligence rebuild request. */
export interface CoreIntelligenceQueueReport {
  jobId: number
  state: string
  notes: string[]
}

/** Structured window metadata used by one Core Intelligence section response. */
export type CoreIntelligenceSectionWindow =
  | {
      kind: 'date-range'
      dateRange: DateRange
    }
  | {
      kind: 'calendar-day-history'
      referenceDate: string
    }

/** Shared evidence/freshness metadata emitted alongside one section payload. */
export interface CoreIntelligenceSectionMeta {
  sectionId: string
  generatedAt?: string | null
  window: CoreIntelligenceSectionWindow
  moduleIds: string[]
  sourceTables: string[]
  includesEnrichment: boolean
  state: 'ready' | 'stale' | 'disabled' | 'degraded'
  stateReason?: string | null
  notes: string[]
}

/** Generic transport envelope for `/intelligence` section payloads. */
export interface CoreIntelligenceSectionResult<T> {
  data: T
  meta: CoreIntelligenceSectionMeta
}

/** Timed sample for one section inside a staged overview batch. */
export interface CoreIntelligenceSectionTiming {
  sectionId: string
  durationMs: number
}

/** Stage-by-stage timing summary emitted for full Core Intelligence rebuilds. */
export interface CoreIntelligenceStageTimings {
  visitDeriveMs: number
  dailyRollupMs: number
  structuralRebuildMs: number
  totalMs: number
}

/** Shareable/embed-oriented card payload from backend-only provider commands. */
export interface IntelligenceEmbedCardPayload {
  cardId: string
  cardType: string
  title: string
  eyebrow?: string | null
  body: string
  metricLabel?: string | null
  metricValue?: string | null
  href?: string | null
  internalOnly: boolean
}

/** Compact widget snapshot built from aggregate Core Intelligence read models. */
export interface IntelligenceWidgetSnapshot {
  generatedAt: string
  dateRange: DateRange
  digestSummary: DigestSummary
  highlights: IntelligenceEmbedCardPayload[]
  notes: string[]
}

/** Redacted public snapshot that intentionally omits visit-level drilldown fields. */
export interface IntelligencePublicSnapshot {
  generatedAt: string
  dateRange: DateRange
  digestSummary: DigestSummary
  topDomains: string[]
  searchEngines: EngineRanking[]
  discoveryTrend: DiscoveryTrend
  notes: string[]
}

/** One generated file belonging to a reusable local host artifact. */
export interface IntelligenceLocalHostGeneratedFile {
  relativePath: string
  absolutePath?: string | null
  purpose: string
  contents: string
}

/** Request payload for deterministic local-host preview/build commands. */
export interface IntelligenceLocalHostRequest {
  dateRange: DateRange
  profileId?: string | null
  locale: string
}

/** Machine-readable bundle persisted beside one local host artifact. */
export interface IntelligenceLocalHostBundle {
  bundleVersion: string
  hostId: string
  generatedAt: string
  locale: string
  dateRange: DateRange
  profileId?: string | null
  embedCards: IntelligenceEmbedCardPayload[]
  widgetSnapshot: IntelligenceWidgetSnapshot
  publicSnapshot: IntelligencePublicSnapshot
  trustedOnlyCardIds: string[]
  trustedOnlyCardCount: number
  boundaryNotes: string[]
}

/** Existing installed local host discovered on disk for verify UI. */
export interface IntelligenceInstalledLocalHost {
  artifactRoot: string
  entryFilePath: string
  bundle: IntelligenceLocalHostBundle
}

/** Preview payload for one deterministic local host without writing files yet. */
export interface IntelligenceLocalHostPreview {
  artifactRoot: string
  entryFilePath: string
  generatedFiles: IntelligenceLocalHostGeneratedFile[]
  bundle: IntelligenceLocalHostBundle
  boundaryNotes: string[]
  manualSteps: string[]
  warnings: string[]
  installedHost?: IntelligenceInstalledLocalHost | null
}

/** Result payload after writing one deterministic local host artifact. */
export interface IntelligenceLocalHostBuildResult {
  artifactRoot: string
  entryFilePath: string
  generatedFiles: IntelligenceLocalHostGeneratedFile[]
  bundle: IntelligenceLocalHostBundle
  boundaryNotes: string[]
  manualSteps: string[]
  warnings: string[]
  installedHost?: IntelligenceInstalledLocalHost | null
}

/** Trend direction indicator derived from period-over-period comparison. */
export type TrendDirection = 'up' | 'down' | 'flat'

/** A numeric KPI with optional period-over-period comparison. */
export interface KpiMetric {
  value: number
  previousValue?: number | null
  changePercent?: number | null
  trend: TrendDirection
}

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
}

/** First-band `/intelligence` payload used to keep route entry responsive. */
export interface CoreIntelligencePrimaryOverview {
  digestSummary: CoreIntelligenceSectionResult<DigestSummary>
  onThisDay: CoreIntelligenceSectionResult<OnThisDayEntry[]>
  topSites: CoreIntelligenceSectionResult<TopSite[]>
  refindPages: CoreIntelligenceSectionResult<RefindPage[]>
  searchEngineRanking: CoreIntelligenceSectionResult<EngineRanking[]>
  topSearchConcepts: CoreIntelligenceSectionResult<SearchConcept[]>
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
  stableSources: CoreIntelligenceSectionResult<StableSource[]>
  searchEffectiveness: CoreIntelligenceSectionResult<SearchEffectiveness>
  frictionSignals: CoreIntelligenceSectionResult<FrictionSignal[]>
  reopenedInvestigations: CoreIntelligenceSectionResult<ReopenedInvestigation[]>
  discoveryTrendWeek: CoreIntelligenceSectionResult<DiscoveryTrend>
  breadthIndex: CoreIntelligenceSectionResult<BreadthIndex>
  pathFlows: CoreIntelligenceSectionResult<PathFlow[]>
  compareSets: CoreIntelligenceSectionResult<CompareSet[]>
  multiBrowserDiff: CoreIntelligenceSectionResult<BrowserDiff>
  observedInteractions: CoreIntelligenceSectionResult<ObservedInteraction[]>
  timings: CoreIntelligenceSectionTiming[]
  totalDurationMs: number
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
}

/** Token frequency for word cloud */
export interface SearchConcept {
  term: string
  frequency: number
  /** Which engines contributed to this term */
  engines: string[]
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
  page: number
  pageSize: number
}

// ---------------------------------------------------------------------------
// 2.3 Refind Pages (常重找的頁面)
// ---------------------------------------------------------------------------

export interface RefindPage {
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

// ---------------------------------------------------------------------------
// 3.1 Browsing Sessions (瀏覽會話)
// ---------------------------------------------------------------------------

export interface SessionSummary {
  sessionId: string
  firstVisitMs: number
  lastVisitMs: number
  visitCount: number
  searchCount: number
  domainCount: number
  isDeepDive: boolean
  autoTitle?: string | null
}

/** Paginated sessions list */
export interface SessionListResult {
  sessions: SessionSummary[]
  total: number
  page: number
  pageSize: number
}

/** Detailed session with visit list */
export interface SessionDetail {
  session: SessionSummary
  visits: SessionVisit[]
  trails: TrailSummary[]
}

export interface SessionVisit {
  visitId: number
  url: string
  title?: string | null
  registrableDomain: string
  visitTimeMs: number
  isSearchEvent: boolean
  searchQuery?: string | null
  searchEngine?: string | null
  trailId?: string | null
  transitionType?: string | null
}

// ---------------------------------------------------------------------------
// 3.2 Search Trails (搜索旅程)
// ---------------------------------------------------------------------------

export interface TrailSummary {
  trailId: string
  sessionId?: string | null
  initialQuery: string
  searchEngine: string
  reformulationCount: number
  visitCount: number
  landingUrl?: string | null
  landingDomain?: string | null
  firstVisitMs: number
  lastVisitMs: number
  maxDepth: number
  queries: string[]
}

export interface TrailListResult {
  trails: TrailSummary[]
  total: number
  page: number
  pageSize: number
}

export interface TrailDetail {
  trail: TrailSummary
  members: TrailMember[]
}

export interface TrailMember {
  trailId: string
  visitId: number
  ordinal: number
  role: 'search_event' | 'click' | 'landing'
  url: string
  title?: string | null
  visitTimeMs: number
  searchQuery?: string | null
}

// ---------------------------------------------------------------------------
// 3.3 Navigation Path Tracer (導航溯源)
// ---------------------------------------------------------------------------

export interface NavigationPathStep {
  visitId: number
  url: string
  title?: string | null
  visitTimeMs: number
  depth: number
}

export interface NavigationPath {
  targetVisitId: number
  steps: NavigationPathStep[]
}

export interface HubPage {
  url: string
  title?: string | null
  registrableDomain: string
  /** How many trails' navigation paths include this URL */
  trailReferenceCount: number
}

// ---------------------------------------------------------------------------
// 4.1 Domain Deep Dive (網站深度分析)
// ---------------------------------------------------------------------------

export interface DomainDeepDive {
  registrableDomain: string
  displayName?: string | null
  domainCategory: string
  totalVisits: number
  activeDays: number
  trailCount: number
  arrivalBreakdown: ArrivalBreakdown
  topPages: DomainPageStat[]
  topReferrers: DomainFlowStat[]
  topExits: DomainFlowStat[]
  visitTrend: DomainTrendPoint[]
}

export interface ArrivalBreakdown {
  search: number
  link: number
  typed: number
  other: number
}

export interface DomainPageStat {
  path: string
  visitCount: number
}

export interface DomainFlowStat {
  domain: string
  displayName?: string | null
  count: number
}

// ---------------------------------------------------------------------------
// 4.2 Stable Answer Sources (穩定答案來源)
// ---------------------------------------------------------------------------

export interface StableSource {
  registrableDomain: string
  displayName?: string | null
  sourceRole: 'entry' | 'landing' | 'reference'
  trailCount: number
  stableLandingCount: number
  effectivenessScore: number
}

// ---------------------------------------------------------------------------
// 4.3 Search Effectiveness (搜索效率分析)
// ---------------------------------------------------------------------------

export interface SearchEffectiveness {
  engineStats: EngineEffectiveness[]
  topResolvingSources: StableSource[]
  hardestTopics: HardTopic[]
}

export interface EngineEffectiveness {
  searchEngine: string
  displayName?: string | null
  avgReformulations: number
  totalTrails: number
  avgDepth: number
}

export interface HardTopic {
  queryFamily: string
  reformulationCount: number
  reSearchLagDays: number
}

// ---------------------------------------------------------------------------
// 4.4 Friction Detection (碰壁偵測)
// ---------------------------------------------------------------------------

export type FrictionEvidenceType = 'strong' | 'weak'

export interface FrictionSignal {
  registrableDomain?: string | null
  url?: string | null
  evidenceType: FrictionEvidenceType
  /** E.g. 'http_error', 'bounce_pattern', 'excessive_reformulation' */
  signalKind: string
  occurrenceCount: number
  description: string
}

// ---------------------------------------------------------------------------
// 4.5 Reopened Investigations (反覆查的問題)
// ---------------------------------------------------------------------------

export interface ReopenedInvestigation {
  investigationId: string
  anchorType: 'query_family' | 'reference_page'
  anchorId: string
  anchorLabel: string
  occurrenceCount: number
  distinctDays: number
  firstSeenAt: string
  lastSeenAt: string
}

// ---------------------------------------------------------------------------
// 4.6 Browsing Rhythm Heatmap (瀏覽節奏熱圖)
// ---------------------------------------------------------------------------

export interface RhythmHeatmapCell {
  /** Day of week: 0 (Sun) – 6 (Sat) */
  dow: number
  /** Hour: 0 – 23 */
  hour: number
  visitCount: number
}

export interface RhythmHeatmap {
  cells: RhythmHeatmapCell[]
  maxCount: number
}

// ---------------------------------------------------------------------------
// 4.7 Discovery Trend (探索率趨勢)
// ---------------------------------------------------------------------------

export interface DiscoveryTrendPoint {
  dateKey: string
  discoveryRate: number
  newDomainCount: number
  totalVisits: number
}

export interface DiscoveryTrend {
  points: DiscoveryTrendPoint[]
  availableYears: number[]
}

// ---------------------------------------------------------------------------
// 4.8 Activity Mix (使用構成)
// ---------------------------------------------------------------------------

export interface CategoryMixEntry {
  domainCategory: string
  visitCount: number
  share: number
}

export interface ActivityMix {
  categories: CategoryMixEntry[]
  changeVsPrevious: CategoryChangeEntry[]
}

export interface CategoryChangeEntry {
  domainCategory: string
  currentShare: number
  previousShare: number
  changePoints: number
}

export interface ActivityMixTrendPoint {
  dateKey: string
  categories: CategoryMixEntry[]
}

export interface ActivityMixTrend {
  points: ActivityMixTrendPoint[]
}

// ---------------------------------------------------------------------------
// 4.9 Breadth Index (集中度分析)
// ---------------------------------------------------------------------------

export interface BreadthIndex {
  /** HHI score: 0 (dispersed) – 1 (concentrated) */
  hhi: number
  /** 0–100 score derived from HHI */
  breadthScore: number
  /** "50% of your browsing is concentrated in X domains" */
  concentrationDomainCount: number
}

// ---------------------------------------------------------------------------
// 4.10 Deep Dive Session
// ---------------------------------------------------------------------------

// Reuses `SessionSummary.isDeepDive` flag — no separate type needed.

// ---------------------------------------------------------------------------
// 4.11 Path Flows (常見路線)
// ---------------------------------------------------------------------------

export interface PathFlow {
  flowPattern: string
  stepCount: number
  occurrenceCount: number
  lastSeenAt: string
}

// ---------------------------------------------------------------------------
// 4.12 Compare Sets (比較頁面組)
// ---------------------------------------------------------------------------

export interface CompareSet {
  compareSetId: string
  searchQuery: string
  pages: CompareSetPage[]
}

export interface CompareSetPage {
  url: string
  title?: string | null
  registrableDomain: string
  visitCount: number
  isLanding: boolean
}

// ---------------------------------------------------------------------------
// 4.13 Multi-Browser Diff (多瀏覽器對比)
// ---------------------------------------------------------------------------

export interface BrowserDiff {
  profiles: BrowserProfileSummary[]
  exclusiveDomains: ExclusiveDomainEntry[]
  sharedDomains: string[]
  categoryDistributions: BrowserCategoryDistribution[]
}

export interface BrowserProfileSummary {
  profileId: string
  profileName: string
  browserFamily: string
  domainCount: number
  visitCount: number
}

export interface ExclusiveDomainEntry {
  registrableDomain: string
  profileId: string
  visitCount: number
}

export interface BrowserCategoryDistribution {
  profileId: string
  profileName: string
  categories: CategoryMixEntry[]
}

// ---------------------------------------------------------------------------
// 4.14 Observed Interactions (瀏覽器直接報告的互動數據)
// ---------------------------------------------------------------------------

export interface ObservedInteraction {
  visitId: number
  url: string
  title?: string | null
  /** Source browser family */
  browserFamily: string
  foregroundDurationMs?: number | null
  scrollingTimeMs?: number | null
  scrollingDistance?: number | null
  keyPresses?: number | null
  typingTimeMs?: number | null
  loadSuccessful?: boolean | null
  pageEndReason?: string | null
}

// ---------------------------------------------------------------------------
// 4.A Explainability Panel (可解釋性)
// ---------------------------------------------------------------------------

export interface ExplainabilityFactor {
  label: string
  rawValue: number
  weight: number
  contribution: number
}

export interface Explanation {
  entityType: string
  entityId: string
  triggerRule: string
  factors: ExplainabilityFactor[]
  participatingVisitIds: number[]
}

// ---------------------------------------------------------------------------
// Time range preset enum
// ---------------------------------------------------------------------------

export type TimeRangePreset =
  | 'day'
  | 'week'
  | 'month'
  | 'quarter'
  | 'year'
  | 'custom'
