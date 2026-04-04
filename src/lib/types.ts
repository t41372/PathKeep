export type ArchiveMode = 'Plaintext' | 'Encrypted'
export type LanguagePreference = 'system' | 'en' | 'zh-CN' | 'zh-TW'

export interface RemoteBackupConfig {
  enabled: boolean
  bucket: string
  region: string
  endpoint?: string | null
  prefix: string
  pathStyle: boolean
  uploadAfterBackup: boolean
  credentialsSaved: boolean
  lastUploadedAt?: string | null
  lastUploadedObjectKey?: string | null
  lastError?: string | null
}

export interface AppConfig {
  initialized: boolean
  archiveMode: ArchiveMode
  preferredLanguage: LanguagePreference
  dueAfterHours: number
  scheduleCheckIntervalHours: number
  checkpointDays: number
  captureFavicons: boolean
  selectedProfileIds: string[]
  gitEnabled: boolean
  rememberDatabaseKeyInKeyring: boolean
  appAutostart: boolean
  remoteBackup: RemoteBackupConfig
}

export interface AppDirectories {
  appRoot: string
  configPath: string
  archiveDatabasePath: string
  auditRepoPath: string
  manifestsDir: string
  exportsDir: string
  rawSnapshotsDir: string
  stagingDir: string
  quarantineDir: string
  scheduleDir: string
  strongholdPath: string
  strongholdSaltPath: string
}

export interface ArchiveStatus {
  initialized: boolean
  encrypted: boolean
  unlocked: boolean
  databasePath: string
  lastSuccessfulBackupAt?: string | null
  warning?: string | null
}

export interface KeyringStatusReport {
  available: boolean
  backend: string
  storedSecret: boolean
  message?: string | null
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
}

export interface BackupRunOverview {
  id: number
  startedAt: string
  finishedAt?: string | null
  status: string
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

export interface AppSnapshot {
  directories: AppDirectories
  config: AppConfig
  archiveStatus: ArchiveStatus
  keyringStatus: KeyringStatusReport
  browserProfiles: BrowserProfile[]
  recentRuns: BackupRunOverview[]
  recentImportBatches: ImportBatchOverview[]
}

export interface HistoryQuery {
  q?: string | null
  profileId?: string | null
  domain?: string | null
  limit?: number | null
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

export interface S3CredentialInput {
  accessKeyId: string
  secretAccessKey: string
}

export interface RemoteBackupPreview {
  bundlePath: string
  objectKey: string
  uploadUrl: string
  previewCommand: string
  manualSteps: string[]
  warnings: string[]
}

export interface RemoteBackupResult {
  uploaded: boolean
  bundlePath: string
  objectKey: string
  uploadUrl: string
  message: string
}

export interface TakeoutRequest {
  sourcePath: string
  dryRun: boolean
}

export interface TakeoutFileReport {
  path: string
  kind: string
  status: string
  records: number
}

export interface TakeoutPreviewEntry {
  sourcePath: string
  url: string
  title?: string | null
  visitedAt: string
  sourceVisitId: number
  status: string
}

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

export interface ImportBatchDetail {
  batch: ImportBatchOverview
  previewEntries: TakeoutPreviewEntry[]
  recognizedFiles: TakeoutFileReport[]
  quarantinedFiles: TakeoutFileReport[]
  notes: string[]
}

export interface TakeoutInspection {
  sourcePath: string
  dryRun: boolean
  recognizedFiles: TakeoutFileReport[]
  quarantinedFiles: TakeoutFileReport[]
  candidateItems: number
  importedItems: number
  duplicateItems: number
  previewEntries: TakeoutPreviewEntry[]
  importBatch?: ImportBatchOverview | null
  notes: string[]
}

export interface GeneratedFile {
  relativePath: string
  absolutePath?: string | null
  purpose: string
  contents: string
}

export interface SchedulePlan {
  platform: string
  label: string
  executablePath: string
  generatedFiles: GeneratedFile[]
  manualSteps: string[]
  applyCommands: string[][]
  rollbackCommands: string[][]
  applySupported: boolean
}

export interface ApplyResult {
  applied: boolean
  platform: string
  files: string[]
  auditPath?: string | null
  message: string
}

export interface HealthCheck {
  name: string
  ok: boolean
  detail: string
}

export interface HealthReport {
  generatedAt: string
  checks: HealthCheck[]
}

export interface RekeyRequest {
  newMode: ArchiveMode
  newKey?: string | null
}
