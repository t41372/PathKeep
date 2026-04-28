/**
 * @file types-primitives.ts
 * @description Shared Core Intelligence primitives, section envelopes, rebuild contracts, and time-range presets reused across every deterministic route surface.
 * @module core-intelligence/types
 *
 * ## Responsibilities
 * - Own the transport-neutral primitives that other Core Intelligence type modules build on.
 * - Keep rebuild, section metadata, shared-entity, and time-range contracts in one canonical owner.
 *
 * ## Not responsible for
 * - Defining route-specific deterministic payload shapes such as sessions, trails, or domain deep dives.
 * - Owning local-host artifact payloads or share/export snapshots.
 *
 * ## Dependencies
 * - Consumed by every other module under src/lib/core-intelligence/types-*.ts.
 * - Re-exported through src/lib/core-intelligence/types.ts to preserve the public import surface.
 *
 * ## Performance notes
 * - Type-only module; keeping primitives centralized reduces duplicate generic wrapper definitions across hot deterministic surfaces.
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

/** Reusable shared-entity reference carried by trusted output payloads. */
export type InsightEntityReference =
  | {
      kind: 'day'
      date: string
    }
  | {
      kind: 'domain'
      domain: string
    }
  | {
      kind: 'queryFamily'
      familyId: string
    }
  | {
      kind: 'refindPage'
      canonicalUrl: string
    }
  | {
      kind: 'session'
      sessionId: string
    }
  | {
      kind: 'trail'
      trailId: string
    }
  | {
      kind: 'compareSet'
      compareSetId: string
    }

/** Stage-by-stage timing summary emitted for full Core Intelligence rebuilds. */
export interface CoreIntelligenceStageTimings {
  visitDeriveMs: number
  dailyRollupMs: number
  structuralRebuildMs: number
  totalMs: number
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
// Time range preset enum
// ---------------------------------------------------------------------------

export type TimeRangePreset =
  | 'day'
  | 'week'
  | 'month'
  | 'quarter'
  | 'year'
  | 'all'
  | 'custom'
