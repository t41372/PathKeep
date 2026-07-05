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
  /**
   * Present on `failed` runs — the backend-reported reason the run did not
   * complete. Null on successful or skipped runs.
   */
  errorMessage?: string | null
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
  /**
   * Capped excerpt of the matched URL's enrichment text (W-ENRICH-1, 06 §6).
   * Present only on lexical-search results whose `search_documents` row carries
   * enrichment (a content-fetch summary + GitHub topics/desc); absent on plain
   * browse, regex, fuzzy, and preview-fixture rows. The Search result row
   * highlights query terms inside it and suppresses the affordance when empty.
   */
  enrichmentExcerpt?: string | null
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

/**
 * Coverage reported by `get_og_image_coverage_stats`: how many web pages carry a
 * preview image. Raw counts — the UI derives the percentages so it can show both
 * coverage (of all eligible pages) and the success rate (of pages checked).
 */
export interface OgImageCoverageStats {
  /** Distinct https pages in the archive (the eligible denominator; http is never fetched). */
  eligiblePages: number
  /** Pages an og:image fetch has been attempted for. */
  attemptedPages: number
  /** Pages with a successfully fetched og:image. */
  pagesWithImage: number
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

/**
 * Enumerates the known reasons an archive may need launch-time recovery.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type ArchiveRecoveryKind =
  | 'interruptedImportModeDrift'
  | 'interruptedRekeyUnresolved'
  | 'interruptedRestoreUnresolved'
  | 'atRestDriftUnresolved'

/**
 * Describes one verified full-archive safety snapshot available for restore.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RecoverySnapshot {
  id: string
  path: string
  createdAt?: string | null
  sizeBytes: number
  verifiedOpenable: boolean
  /**
   * At-rest signal (keyless): the snapshot is SQLCipher-encrypted on disk. When true the recovery UI
   * must collect the archive key before it can verify/restore — `verifiedOpenable` is size-only here.
   */
  encrypted: boolean
  sourceOp: string
  label: string
}

/**
 * Represents the outcome of a one-click full-archive restore from a safety snapshot.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface FullArchiveRestoreReport {
  runId?: number | null
  restoredSnapshotPath: string
  restoredMode: ArchiveMode
  quarantineDir: string
  sourceEvidenceRebuilt: boolean
  warnings: string[]
}

/**
 * Describes the structured diagnostic the backend surfaces when the archive cannot be opened on launch.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ArchiveRecoveryReport {
  kind: ArchiveRecoveryKind
  configMode: ArchiveMode
  historyVaultMode?: ArchiveMode | null
  sourceEvidenceMode?: ArchiveMode | null
  availableSnapshots: string[]
  recoverySnapshots: RecoverySnapshot[]
  detail: string
}

/**
 * Names the distinct heavy phases a first-run v0.2.0 → v0.3.0 archive upgrade
 * migration walks through inside `initialize_archive`.
 *
 * Mirrors `vault_core::ArchiveUpgradePhase` (serde camelCase). The shell maps
 * each value to a localized `archiveUpgrade.phase.{...}` label so the upgrade
 * screen can name its current phase honestly instead of showing an opaque
 * busy overlay. `intelligence` forward-applies lazily outside the upgrade path,
 * so it is surfaced as an informational line rather than a streamed bar.
 */
export type ArchiveUpgradePhase =
  | 'schemaMigration'
  | 'registrableDomainBackfill'
  | 'searchReprojection'
  | 'intelligence'
  | 'finalizing'

/**
 * One streamed progress tick emitted on `pathkeep://archive-upgrade` while the
 * one-time upgrade migration runs.
 *
 * `processed`/`total` are honest unit counts for phases with a countable unit;
 * opaque single-statement work (index builds / finalize) carries `0/0` as an
 * explicit indeterminate marker. `done` marks the single terminal event so the
 * shell can transition without inferring completion from the counters.
 */
export interface ArchiveUpgradeProgress {
  phase: ArchiveUpgradePhase
  phaseLabel: string
  processed: number
  total: number
  done: boolean
}

/**
 * One phase entry in the cheap upgrade pre-check breakdown.
 *
 * `streamed` is `true` for phases that emit live ticks (schema/backfill/
 * reprojection) and `false` for `intelligence`, which the shell renders as an
 * informational line instead of a bar stuck at zero. `estimatedTotal` seeds the
 * progress UI before the first live tick arrives.
 */
export interface ArchiveUpgradePhaseAssessment {
  phase: ArchiveUpgradePhase
  phaseLabel: string
  pending: boolean
  streamed: boolean
  estimatedTotal: number
}

/**
 * Result of the cheap first-run upgrade pre-check (`assess_archive_upgrade`).
 *
 * The shell reads `pending` at bootstrap to decide whether to show the upgrade
 * screen at all: a fresh install or already-migrated archive reports
 * `pending === false` (no screen), while a genuine version-behind archive
 * reports `pending === true` plus the per-phase breakdown seeding the UI.
 */
export interface ArchiveUpgradeAssessment {
  pending: boolean
  currentSchemaVersion: number
  targetSchemaVersion: number
  phases: ArchiveUpgradePhaseAssessment[]
}
