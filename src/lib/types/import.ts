/**
 * This module defines typed front-end contracts for import inspection, batch review, and rollback follow-through.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `TakeoutRequest`
 * - `TakeoutFileReport`
 * - `TakeoutPreviewEntry`
 * - `ImportBatchOverview`
 * - `ImportBatchDetail`
 * - `TakeoutInspection`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface TakeoutRequest {
  sourcePath: string
  dryRun: boolean
}

/**
 * Describes a browser-direct import request.
 *
 * Browser Direct uses the same preview/import review models as Takeout, but it
 * carries browser profile metadata so the backend can preserve the source
 * profile instead of routing a local SQLite database through the Takeout parser.
 */
export interface BrowserHistoryImportRequest {
  sourcePath: string
  dryRun: boolean
  browserFamily?: string | null
  profileId?: string | null
  browserName?: string | null
  profileName?: string | null
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface TakeoutFileReport {
  path: string
  kind: string
  status: string
  records: number
  classification: string
  reasonCode?: string | null
  reasonDetail?: string | null
  detectedLocale?: string | null
}

/**
 * Defines the typed shape for takeout preview entry.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface TakeoutPreviewEntry {
  sourcePath: string
  url: string
  title?: string | null
  visitedAt: string
  sourceVisitId: number
  status: string
}

/**
 * Defines the typed shape for import batch overview.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ImportBatchOverview {
  id: number
  sourceKind: string
  sourcePath: string
  profileId: string
  createdAt: string
  importedAt?: string | null
  revertedAt?: string | null
  status: string
  candidateItems: number
  importedItems: number
  duplicateItems: number
  visibleItems: number
  auditPath?: string | null
  gitCommit?: string | null
}

/**
 * Represents the detailed view model for import batch.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ImportBatchDetail {
  batch: ImportBatchOverview
  previewEntries: TakeoutPreviewEntry[]
  recognizedFiles: TakeoutFileReport[]
  quarantinedFiles: TakeoutFileReport[]
  notes: string[]
  detectedLocale?: string | null
  previewRangeStart?: string | null
  previewRangeEnd?: string | null
}

/**
 * Defines the typed shape for takeout inspection.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface TakeoutInspection {
  dryRun: boolean
  sourcePath: string
  recognizedFiles: TakeoutFileReport[]
  quarantinedFiles: TakeoutFileReport[]
  previewEntries: TakeoutPreviewEntry[]
  candidateItems: number
  importedItems: number
  duplicateItems: number
  notes: string[]
  importBatch?: ImportBatchOverview | null
  detectedLocale?: string | null
  previewRangeStart?: string | null
  previewRangeEnd?: string | null
}

/**
 * Defines the typed shape for import progress event.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ImportProgressEvent {
  phase: string
  label: string
  detail: string
  current: number
  total: number
  progressPercent?: number | null
  logLines: string[]
  sourcePath?: string | null
  sourceLabel?: string | null
  processedRecords?: number | null
  totalRecords?: number | null
  importedRecords?: number | null
  duplicateRecords?: number | null
  skippedRecords?: number | null
}
