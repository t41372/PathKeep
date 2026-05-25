/**
 * This module defines typed front-end contracts for archive status, recall surfaces, and backup execution.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `ArchiveMode`
 * - `ArchiveStatus`
 * - `BrowserProfile`
 * - `BrowserRetentionBoundary`
 * - `BackupRunOverview`
 * - `BackupProfileSummary`
 * - `BackupReport`
 * - `BackupProgressEvent`
 * - `StorageSummary`
 * - `DashboardSnapshot`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

import type { ProgressLogEvent } from './import'

/**
 * Enumerates the supported archive modes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type ArchiveMode = 'Plaintext' | 'Encrypted'
/**
 * Represents a read model or status snapshot for archive.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ArchiveStatus {
  initialized: boolean
  encrypted: boolean
  unlocked: boolean
  databasePath: string
  lastSuccessfulBackupAt?: string | null
  warning?: string | null
}
/**
 * Defines the typed shape for browser profile.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface BrowserProfile {
  profileId: string
  profileName: string
  browserFamily: string
  browserName: string
  userName?: string | null
  profilePath: string
  historyPath?: string | null
  faviconsPath?: string | null
  historyExists: boolean
  historyReadable?: boolean
  accessIssue?: string | null
  browserVersion?: string | null
  historyFileName: string
  historyBytes: number
  faviconsBytes: number
  supportingBytes: number
  retentionBoundary: BrowserRetentionBoundary
}

/**
 * Defines the typed shape for browser retention boundary.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface BrowserRetentionBoundary {
  kind: 'browser-managed' | 'macos-safari'
  localDays?: number | null
}

/**
 * Defines the typed shape for backup run overview.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface BackupRunOverview {
  id: number
  startedAt: string
  finishedAt?: string | null
  status: string
  runType?: string
  trigger?: string
  profileScope?: string[]
  manifestHash?: string | null
  profilesProcessed: number
  newVisits: number
  newUrls: number
  newDownloads: number
}

/**
 * Represents a condensed summary for backup profile.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface BackupProfileSummary {
  profileId: string
  newVisits: number
  newUrls: number
  newDownloads: number
  checkpointCreated: boolean
  notes: string[]
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface BackupReport {
  dueSkipped: boolean
  reason?: string | null
  run?: BackupRunOverview | null
  profiles: BackupProfileSummary[]
  manifestPath?: string | null
  gitCommit?: string | null
  warnings: string[]
}

/**
 * Defines the typed shape for backup progress event.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface BackupProgressEvent {
  phase: string
  label: string
  detail: string
  step: number
  totalSteps: number
  completedProfiles: number
  totalProfiles: number
  profileId?: string | null
  progressCurrent?: number | null
  progressTotal?: number | null
  progressPercent?: number | null
  logLines?: string[]
  logEvents?: ProgressLogEvent[]
  sourceLabel?: string | null
  processedRecords?: number | null
  totalRecords?: number | null
  importedRecords?: number | null
  duplicateRecords?: number | null
  skippedRecords?: number | null
}

/**
 * Represents a condensed summary for storage.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface StorageSummary {
  archiveDatabaseBytes: number
  sourceEvidenceDatabaseBytes: number
  searchDatabaseBytes: number
  intelligenceDatabaseBytes: number
  manifestBytes: number
  snapshotBytes: number
  exportBytes: number
  stagingBytes: number
  quarantineBytes: number
  semanticSidecarBytes: number
  intelligenceBlobBytes: number
}

/**
 * Defines the typed shape for dashboard snapshot.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface DashboardSnapshot {
  generatedAt: string
  totalProfiles: number
  totalUrls: number
  totalVisits: number
  totalDownloads: number
  lastSuccessfulBackupAt?: string | null
  /**
   * Earliest visible visit_time across the archive. Populated together with
   * `latestVisitAt`; both `null` when the archive has zero rows. The dashboard
   * "Span" stat uses these to label archive coverage rather than the time
   * since last backup.
   */
  earliestVisitAt?: string | null
  latestVisitAt?: string | null
  recentRuns: BackupRunOverview[]
  storage: StorageSummary
  nextAction?: string | null
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface SnapshotRestoreRequest {
  snapshotPath: string
}

/**
 * Represents the preview payload shown before a write or high-risk action happens.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface SnapshotRestorePreview {
  snapshotPath: string
  snapshotKind: string
  sourceRunId?: number | null
  sourceProfileId?: string | null
  sourceBrowserName?: string | null
  createdAt?: string | null
  reason?: string | null
  executeSupported: boolean
  estimatedVisits: number
  estimatedUrls: number
  estimatedDownloads: number
  warnings: string[]
}

/**
 * Defines the typed shape for retention bucket.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RetentionBucket {
  id: string
  bytes: number
  itemCount: number
  paths: string[]
}

/**
 * Represents the preview payload shown before a write or high-risk action happens.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RetentionPreview {
  buckets: RetentionBucket[]
  warnings: string[]
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RetentionPruneRequest {
  bucketIds: string[]
}

/**
 * Defines the typed shape for retention prune result.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RetentionPruneResult {
  runId?: number | null
  deletedBytes: number
  deletedFiles: number
  buckets: RetentionBucket[]
  warnings: string[]
}

/**
 * Defines the typed shape for history query.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface HistoryQuery {
  q?: string | null
  profileId?: string | null
  browserKind?: string | null
  domain?: string | null
  startTimeMs?: number | null
  endTimeMs?: number | null
  sort?: 'relevance' | 'newest' | 'oldest' | null
  limit?: number | null
  page?: number | null
  cursor?: string | null
  regexMode?: boolean
}

/**
 * Defines the typed shape for history entry.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface HistoryEntry {
  id: number
  profileId: string
  url: string
  title?: string | null
  domain: string
  favicon?: {
    dataUrl: string
  } | null
  /**
   * Page-level og:image preview hydrated by the card-mode lookup hook.
   * Distinct from {@link favicon}; either may be present independently.
   */
  ogImage?: {
    dataUrl: string
  } | null
  visitedAt: string
  visitTime: number
  durationMs?: number | null
  transition?: number | null
  sourceVisitId: number
  appId?: string | null
}

/**
 * Defines one batch favicon lookup entry for Explorer rows that have already
 * rendered without blocking on image payloads.
 *
 * The lookup stays URL/profile-scoped so the UI can hydrate icons after first
 * paint without re-encoding favicon bytes inside the main history query.
 */
export interface HistoryFaviconLookupEntry {
  profileId: string
  url: string
  visitTime: number
}

/**
 * Defines one resolved favicon payload returned by the lazy Explorer icon path.
 *
 * Returning the requested URL/profile pair keeps the response deterministic and
 * lets the client cache icon lookups without inventing a second identity model.
 */
export interface HistoryFaviconLookupResult {
  profileId: string
  url: string
  visitTime: number
  favicon?: {
    dataUrl: string
  } | null
}

/**
 * Defines one batch og:image lookup entry. Card-mode hydration keys by page
 * URL only — og:image describes the page itself, not the visit, and the
 * cache is shared across browser profiles.
 */
export interface HistoryOgImageLookupEntry {
  url: string
}

/**
 * Defines one resolved og:image payload returned by the lazy card-mode lookup.
 *
 * `fetchStatus` is one of `ok | missing | http_error | parse_error |
 * too_large | unsupported_mime | blocked | pending`. The frontend renders
 * the og:image bytes only when status is `ok`; other statuses tell the UI
 * whether to fall back to the favicon, the domain swatch, or a "block
 * lifted" hint.
 */
export interface HistoryOgImageLookupResult {
  url: string
  ogImage?: {
    dataUrl: string
  } | null
  fetchStatus: string
}

/** Cache footprint reported by `get_og_image_storage_stats`. */
export interface OgImageStorageStats {
  rowCount: number
  blobCount: number
  totalBytes: number
  oldestFetchedAt?: string | null
}

/** Outcome of one cleanup pass. */
export interface OgImageCleanupReport {
  deletedRows: number
  deletedBlobs: number
  reclaimedBytes: number
}

/** User-pickable eviction mode for the og:image cache. Default is Off. */
export type OgImageCleanupMode =
  | { mode: 'off' }
  | { mode: 'timeTtl'; maxAgeDays: number }
  | { mode: 'sizeCap'; maxBytes: number }
  | { mode: 'lru'; maxBytes: number }

/**
 * How aggressively the og:image worker fetches link-preview bytes.
 * Mirrors `vault_core::OgImageFetchMode`.
 *
 * - `off`        — no fetching anywhere.
 * - `on_demand`  — fetch only when a card-mode row scrolls into view.
 * - `background` — `on_demand` + per-backup new-visit prefetch + the
 *                  daily negative-cache retry. (Default.)
 */
export type OgImageFetchMode = 'off' | 'on_demand' | 'background'

/** Persisted Settings → Storage → Link previews block. */
export interface OgImageSettings {
  fetchEnabled: boolean
  fetchMode: OgImageFetchMode
  /** Per-day cap on negative-cache retry sweeps. Default 50. */
  dailyRefetchBudget: number
  /** Per-backup cap on the new-visit prefetch sweep. Default 100; 0 disables. */
  newVisitPrefetchBudget: number
  blockedHosts: string[]
  cleanup: OgImageCleanupMode
}

/**
 * Describes a response payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface HistoryQueryResponse {
  total: number
  items: HistoryEntry[]
  page: number
  pageSize: number
  pageCount: number
  hasPrevious: boolean
  hasNext: boolean
  nextCursor?: string | null
}

/**
 * Defines the type-level contract for export format.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type ExportFormat = 'html' | 'markdown' | 'text' | 'jsonl'

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ExportRequest {
  query: HistoryQuery
  format: ExportFormat
}

/**
 * Defines the typed shape for export result.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ExportResult {
  format: ExportFormat
  path: string
  count: number
}
