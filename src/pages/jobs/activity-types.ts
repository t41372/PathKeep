/**
 * @file activity-types.ts
 * @description Pure TypeScript type definitions for the Activity center data model.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Define the canonical Activity shape consumed by the Activity center page and its sub-zones.
 * - Keep types data-only so they can be imported from both React components and pure adapters.
 *
 * ## Not responsible for
 * - Building or transforming Activity objects (see activity-adapter.ts).
 * - Rendering or displaying activities (see zone components).
 */

export type ActivityKind =
  | 'index-build'
  | 'model-download'
  | 'content-fetch'
  | 're-embed'
  | 'deterministic-rebuild'
  | 'import'
  | 'backup'

export type ActivityState =
  | 'running'
  | 'queued'
  | 'failed'
  | 'stale'
  | 'succeeded'
  | 'cancelled'

export type InterruptionResumability = 'safe' | 'per-file' | 'cannot-resume'

export interface ActivityProgress {
  /** 0–1 clamped, or null for indeterminate */
  value: number | null
  /**
   * Raw label string.
   * - kind='embedded': the embedded page count as a stringified number
   * - kind='verbatim': human-readable progress text from the runtime (render as-is)
   * - kind='records': not used (use processedCount/totalCount)
   * - null when no label
   */
  label: string | null
  /**
   * How the zone should render the count label:
   * - 'embedded' → progressEmbeddedLabel {count: label}
   * - 'verbatim'  → render label directly (already human text from runtime)
   * - 'records'   → progressRecordsLabel {processed: processedCount, total: totalCount}
   * - null        → no count label shown
   */
  labelKind: 'embedded' | 'verbatim' | 'records' | null
  /** Used when labelKind='records': number of processed records */
  processedCount?: number | null
  /** Used when labelKind='records': total expected records */
  totalCount?: number | null
}

export interface Activity {
  id: string
  kind: ActivityKind
  state: ActivityState
  taskNameKey: string
  causeKey?: string
  cause?: string
  timestamp: string
  progress: ActivityProgress
  resumability: InterruptionResumability
  aiJobId?: number
  runtimeJobId?: number
  /**
   * Whether a running/queued runtime job can be cancelled from the Activity center. Sourced from
   * `IntelligenceJobOverview.cancellable` so the Running-now zone only offers Cancel "where
   * supported" (honest control surface, not a button that no-ops).
   */
  cancellable?: boolean
  resultLink?: string | null
  outcomeKey?: string
}
