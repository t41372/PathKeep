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
import type { SessionSummary, TrailSummary } from './types-navigation'

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
  familyId: string
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
