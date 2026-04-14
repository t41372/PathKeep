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

import type { RemoteBackupResult } from './remote'

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
  remoteBackup?: RemoteBackupResult | null
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
}

/**
 * Represents a condensed summary for storage.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface StorageSummary {
  archiveDatabaseBytes: number
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
  sort?: 'newest' | 'oldest' | null
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
  visitedAt: string
  visitTime: number
  durationMs?: number | null
  transition?: number | null
  sourceVisitId: number
  appId?: string | null
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
