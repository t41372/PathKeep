/**
 * @file types-navigation.ts
 * @description Deterministic browsing-session, search-trail, and navigation-path contracts reused by Explorer and promoted intelligence routes.
 * @module core-intelligence/types
 *
 * ## Responsibilities
 * - Own session, trail, and navigation-path payload shapes.
 * - Keep browse-first grouped-view entities in one canonical owner.
 *
 * ## Not responsible for
 * - Owning overview KPI or domain-analysis contracts.
 * - Owning output/share/local-host payloads.
 *
 * ## Dependencies
 * - Consumed by explorer grouped panels, promoted entity routes, and analysis detail payloads.
 * - Re-exported through src/lib/core-intelligence/types.ts for stable imports.
 *
 * ## Performance notes
 * - Type-only module; centralizing browse-first entities avoids duplicate trail/session shape drift between Explorer and intelligence routes.
 */

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
  canonicalUrl?: string | null
  title?: string | null
  registrableDomain?: string | null
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
