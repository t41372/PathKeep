/**
 * @file types-analysis.ts
 * @description Deterministic deep-analysis contracts for domains, discovery, concentration, compare sets, browser diffs, and explainability.
 * @module core-intelligence/types
 *
 * ## Responsibilities
 * - Own the below-the-fold analysis payloads used by intelligence deep-dive surfaces.
 * - Keep compare-set, browser-diff, friction, and explainability contracts together.
 *
 * ## Not responsible for
 * - Owning shared transport primitives or rebuild metadata.
 * - Owning session/trail navigation payloads or trusted output bundle wrappers.
 *
 * ## Dependencies
 * - Depends on navigation summaries for compare-set and detail payloads.
 * - Consumed by intelligence sections, promoted routes, and output payload modules.
 *
 * ## Performance notes
 * - Type-only module; isolating the heavier analysis shapes reduces noise when overview-centric routes only need first-band contracts.
 */

import type { DomainTrendPoint } from './types-overview'
import type { Page } from './types-primitives'
import type { SessionSummary, TrailSummary } from './types-navigation'

// ---------------------------------------------------------------------------
// 4.1 Domain Deep Dive (網站深度分析)
// ---------------------------------------------------------------------------

/**
 * One site over a window. Totals, active days and the trend come from the
 * same daily rollups as Top sites, so the numbers agree with it.
 */
export interface DomainDeepDive {
  registrableDomain: string
  displayName?: string | null
  domainCategory: string
  totalVisits: number
  activeDays: number
  /** Distinct URLs on the site visited in the window. */
  pageCount: number
  /** Searches whose trail landed on this site in the window. */
  landingSearchCount: number
  /** Most visited first; one page of them, continued by `nextCursor`. */
  topPages: Page<DomainPageStat>
  /** Most frequent first; one page of them, continued by `nextCursor`. */
  landingSearches: Page<DomainSearchStat>
  /** Days without visits are left out. */
  visitTrend: DomainTrendPoint[]
  arrivals: SiteArrivals
  /** Sites whose links opened this one, most visits first. */
  referrers: SiteFlowStat[]
  /** Search engines whose results opened it, by the engine's site. */
  searchSites: SiteFlowStat[]
  /** Sites opened from links on this one. */
  onward: SiteFlowStat[]
  /**
   * What was searched on the site (its search engines' keyword searches),
   * most often first; empty for a site with no search engine.
   */
  siteSearches: DomainSearchStat[]
  /**
   * The category the user set for the site, as set now; `domainCategory` is
   * the one Insights were built with, which follows once they are rebuilt.
   */
  userCategory?: string | null
  /** The category PathKeep's own lists give the site (`unknown`: none). */
  listCategory?: string
  /**
   * The parts of the site the lists sort on their own (`mail.google.com` in
   * `google.com`), which keep their categories whatever the user sets for the
   * site; only those the archive has pages on.
   */
  listedParts?: SitePart[]
  /** Insights were built with another category for the site than the one set now. */
  categoryPending?: boolean
}

/** A host of a site that PathKeep's lists sort on its own. */
export interface SitePart {
  host: string
  /** The category the lists give it. */
  category: string
}

/**
 * How the visits to a site began: each visit whose opener (the page the
 * browser says opened it) was not on the site.
 */
export interface SiteArrivals {
  /** No opener: typed, a bookmark, a new tab or another app. */
  direct: number
  /** Opened from a search's results. */
  search: number
  /** Opened from a link on another site, or on a page PathKeep did not see. */
  link: number
  /**
   * Visits from browsers that do not record what opened a page (Safari,
   * Google Takeout): how they began is not known.
   */
  unrecorded: number
}

/** Another site, and how many visits went between it and the site. */
export interface SiteFlowStat {
  registrableDomain: string
  visitCount: number
}

export interface DomainPageStat {
  url: string
  title?: string | null
  /** The URL without scheme and host. */
  path: string
  visitCount: number
}

export interface DomainSearchStat {
  query: string
  count: number
}

// ---------------------------------------------------------------------------
// 4.2 Stable Answer Sources (穩定答案來源)
// ---------------------------------------------------------------------------

/** A site searches ended on: where the user's answers come from. */
export interface StableSource {
  registrableDomain: string
  displayName?: string | null
  /** Searches in the range whose landing page was on this site. */
  searchCount: number
  /** How many different searches those were. */
  queryCount: number
}

// ---------------------------------------------------------------------------
// 4.3 Search Effectiveness (搜索效率分析)
// ---------------------------------------------------------------------------

export interface SearchEffectiveness {
  engineStats: EngineEffectiveness[]
  topResolvingSources: Page<StableSource>
  hardestTopics: Page<HardTopic>
}

export interface EngineEffectiveness {
  searchEngine: string
  displayName?: string | null
  avgReformulations: number
  totalTrails: number
  avgDepth: number
}

/**
 * A search the user came back to, as one row across browser profiles and
 * engines: the families that begin with it, their searches summed.
 */
export interface HardTopic {
  /** The key the families share, which tells rows apart. */
  topicKey: string
  queryFamily: string
  /** Their searches, summed. */
  searchCount: number
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
  /** The search as first typed. */
  searchQuery: string
  /** How many times it was reworded. */
  reformulationCount: number
  /** Pages opened from it, the searches included. */
  visitCount: number
}

// ---------------------------------------------------------------------------
// 4.5 Reopened Investigations (反覆查的問題)
// ---------------------------------------------------------------------------

export interface ReopenedInvestigation {
  investigationId: string
  anchorType: 'query_family' | 'reference_page'
  anchorId: string
  anchorLabel: string
  /** The page's site, for its icon; absent for a search. */
  site?: string | null
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
  flowId: string
  flowPattern: string
  stepCount: number
  occurrenceCount: number
  lastSeenAt: string
  steps: PathFlowStep[]
}

export interface PathFlowStep {
  index: number
  label: string
  registrableDomain?: string | null
}

// ---------------------------------------------------------------------------
// 4.12 Compare Sets (比較頁面組)
// ---------------------------------------------------------------------------

export interface CompareSet {
  compareSetId: string
  trailId: string
  searchQuery: string
  pageCategory: string
  pages: CompareSetPage[]
  /** The last time one of the pages was opened in this search. */
  lastVisitedAt: string
}

export interface CompareSetPage {
  canonicalUrl: string
  url: string
  title?: string | null
  registrableDomain: string
  visitCount: number
  isLanding: boolean
}

export interface CompareSetDetail {
  compareSet: CompareSet
  trail: TrailSummary
  session?: SessionSummary | null
  recentDays: string[]
}

// ---------------------------------------------------------------------------
// 4.13 Multi-Browser Diff (多瀏覽器對比)
// ---------------------------------------------------------------------------

export interface BrowserDiff {
  profiles: BrowserProfileSummary[]
  exclusiveDomains: ExclusiveDomainEntry[]
  /** Sites more than one profile visited, most visits (all profiles) first. */
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

/**
 * Stable, locale-independent CODES for the rule that produced an entity.
 *
 * Mirrors the `Explanation::TRIGGER_RULE_*` constants in
 * `vault-core/src/models/core_intelligence/analytics.rs`. The panel localizes from these codes plus
 * the structured params on `Explanation`, so it never has to parse the English `triggerRule` prose
 * to recover a number or an anchor query. `string & {}` keeps an unknown backend code assignable
 * (it falls back to legacy prose matching) while still autocompleting the shipped set.
 */
export type ExplanationTriggerRuleCode =
  | 'refind-score'
  | 'session-deep-dive'
  | 'session-gap'
  | 'search-trail'
  | 'query-family'
  | 'reopened-investigation'
  | 'habit-pattern'
  | 'habit-pattern-interrupted'
  | 'path-flow'
  | 'compare-set'

export interface Explanation {
  entityType: string
  entityId: string
  /**
   * English DIAGNOSTIC prose. OPTIONAL: every shipped backend rule now carries a
   * `triggerRuleCode`, so a front-end-composed explanation (the Refind route hands the panel one
   * directly) has no prose to supply and must not invent an English sentence just to fill this in.
   * The UI renders from `triggerRuleCode`; this string is only matched as a fallback for payloads
   * that predate the code (see `localizeTriggerRule`).
   */
  triggerRule?: string
  /**
   * Stable rule CODE (additive — absent on pre-code payloads). Typed loosely on purpose: a code the
   * front end does not know must degrade to the legacy prose path rather than fail to parse.
   */
  triggerRuleCode?: ExplanationTriggerRuleCode | (string & {}) | null
  /** Score threshold the entity crossed — paired with `refind-score`. */
  triggerRuleScore?: number | null
  /** Anchor query the rule keyed on — paired with `search-trail` and `query-family`. */
  triggerRuleQuery?: string | null
  /** Session-grouping gap threshold in minutes — paired with `session-gap`. */
  triggerRuleGapMinutes?: number | null
  /**
   * Habit-cadence TYPE code (`daily_habit` / `weekly_habit` / `periodic_reference`) — paired with the
   * two `habit-pattern*` codes. The raw code, localized by the panel's habit-type table.
   */
  triggerRuleHabitType?: string | null
  factors: ExplainabilityFactor[]
  participatingVisitIds: number[]
}
