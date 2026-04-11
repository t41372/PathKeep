import type { RemoteBackupResult } from './remote'

export type ArchiveMode = 'Plaintext' | 'Encrypted'
export interface ArchiveStatus {
  initialized: boolean
  encrypted: boolean
  unlocked: boolean
  databasePath: string
  lastSuccessfulBackupAt?: string | null
  warning?: string | null
}
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

export interface BrowserRetentionBoundary {
  kind: 'browser-managed' | 'macos-safari'
  localDays?: number | null
}

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

export interface BackupProfileSummary {
  profileId: string
  newVisits: number
  newUrls: number
  newDownloads: number
  rawRows: number
  checkpointCreated: boolean
  notes: string[]
}

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

export interface StorageSummary {
  archiveDatabaseBytes: number
  manifestBytes: number
  snapshotBytes: number
  exportBytes: number
  stagingBytes: number
  quarantineBytes: number
}

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

export interface SnapshotRestoreRequest {
  snapshotPath: string
}

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

export interface RetentionBucket {
  id: string
  bytes: number
  itemCount: number
  paths: string[]
}

export interface RetentionPreview {
  buckets: RetentionBucket[]
  warnings: string[]
}

export interface RetentionPruneRequest {
  bucketIds: string[]
}

export interface RetentionPruneResult {
  runId?: number | null
  deletedBytes: number
  deletedFiles: number
  buckets: RetentionBucket[]
  warnings: string[]
}

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

export interface HistoryEntry {
  id: number
  profileId: string
  url: string
  title?: string | null
  domain: string
  visitedAt: string
  visitTime: number
  durationMs?: number | null
  transition?: number | null
  sourceVisitId: number
  appId?: string | null
}

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

export type ExportFormat = 'html' | 'markdown' | 'text' | 'jsonl'

export interface ExportRequest {
  query: HistoryQuery
  format: ExportFormat
}

export interface ExportResult {
  format: ExportFormat
  path: string
  count: number
}
