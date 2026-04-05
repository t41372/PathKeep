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

export type AiRequestFormat =
  | 'openai'
  | 'anthropic'
  | 'google'
  | 'ollama'
  | 'lm-studio'

export type AiProviderPurpose = 'llm' | 'embedding'

export interface AiProviderConfig {
  id: string
  name: string
  purpose: AiProviderPurpose
  requestFormat: AiRequestFormat
  enabled: boolean
  baseUrl?: string | null
  apiKeySaved: boolean
  defaultModel: string
  modelCatalog: string[]
  temperature?: number | null
  maxTokens?: number | null
  dimensions?: number | null
  notes?: string | null
}

export interface AiSettings {
  enabled: boolean
  assistantEnabled: boolean
  semanticIndexEnabled: boolean
  mcpEnabled: boolean
  skillEnabled: boolean
  autoIndexAfterBackup: boolean
  llmProviderId?: string | null
  embeddingProviderId?: string | null
  retrievalTopK: number
  assistantSystemPrompt: string
  llmProviders: AiProviderConfig[]
  embeddingProviders: AiProviderConfig[]
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
  ai: AiSettings
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

export interface AppBuildInfo {
  productName: string
  version: string
  gitCommitShort: string
  gitCommitFull: string
  gitDirty: boolean
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

export interface AiIndexStatus {
  enabled: boolean
  assistantEnabled: boolean
  mcpEnabled: boolean
  skillEnabled: boolean
  ready: boolean
  indexedItems: number
  lastIndexedAt?: string | null
  llmProviderId?: string | null
  embeddingProviderId?: string | null
  warning?: string | null
}

export interface InsightStatus {
  ready: boolean
  lastRunAt?: string | null
  runs: number
  cards: number
  topics: number
  threads: number
  contentCoverage: number
  warning?: string | null
}

export interface InsightEvidenceItem {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  visitedAt: string
  note?: string | null
}

export interface InsightCard {
  cardId: string
  kind: string
  title: string
  summary: string
  windowDays: number
  profileId?: string | null
  score: number
  chromiumEnhanced: boolean
  evidence: InsightEvidenceItem[]
}

export interface InsightTopicSummary {
  topicId: string
  label: string
  profileScope: string
  windowDays: number
  firstSeenAt: string
  lastSeenAt: string
  visitCount: number
  revisitCount: number
  trendSlope: number
  burstScore: number
  evidence: InsightEvidenceItem[]
}

export interface InsightThreadSummary {
  threadId: string
  profileId: string
  title: string
  status: string
  firstSeenAt: string
  lastSeenAt: string
  visitCount: number
  reopenCount: number
  openLoopScore: number
  dominantTopicId?: string | null
  chromiumEnhanced: boolean
  evidence: InsightEvidenceItem[]
}

export interface InsightThreadDetail {
  summary: InsightThreadSummary
  visits: InsightEvidenceItem[]
}

export interface InsightQueryLadder {
  rootTerm: string
  profileId: string
  steps: string[]
  stages: string[]
  count: number
  chromiumOnly: boolean
}

export interface InsightWorkflowRole {
  role: string
  count: number
}

export interface InsightWorkflowEdge {
  fromRole: string
  toRole: string
  count: number
}

export interface InsightWorkflowMap {
  profileId?: string | null
  roles: InsightWorkflowRole[]
  edges: InsightWorkflowEdge[]
  chromiumEnhanced: boolean
}

export interface InsightProfileFacet {
  key: string
  label: string
  value: string
  confidence: number
  evidence: InsightEvidenceItem[]
}

export interface InsightSnapshot {
  generatedAt: string
  windowDays: number
  profileId?: string | null
  status: InsightStatus
  cards: InsightCard[]
  topics: InsightTopicSummary[]
  threads: InsightThreadSummary[]
  queryLadders: InsightQueryLadder[]
  workflowMap: InsightWorkflowMap
  profileFacets: InsightProfileFacet[]
  notes: string[]
}

export interface RunInsightsRequest {
  profileId?: string | null
  windowDays?: number | null
  fullRebuild: boolean
  limit?: number | null
}

export interface RunInsightsReport {
  runId: number
  processedVisits: number
  enrichedVisits: number
  failedEnrichments: number
  topicCount: number
  threadCount: number
  cardCount: number
  contentCoverage: number
  lastRunAt: string
  notes: string[]
}

export interface ExplainInsightRequest {
  insightId: string
  insightKind: string
  profileId?: string | null
  windowDays?: number | null
}

export interface InsightExplanation {
  explanation: string
  usedLlm: boolean
  citations: InsightEvidenceItem[]
  notes: string[]
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
  aiStatus: AiIndexStatus
  insightStatus: InsightStatus
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
}

export interface ScheduleGeneratedFile {
  relativePath: string
  absolutePath?: string | null
  purpose: string
  contents: string
}

export interface SchedulePlan {
  platform: string
  label: string
  executablePath: string
  generatedFiles: ScheduleGeneratedFile[]
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
  status: string
  message: string
}

export interface HealthReport {
  generatedAt: string
  checks: HealthCheck[]
}

export interface RekeyRequest {
  newMode: ArchiveMode
  newKey?: string | null
}

export interface AiProviderSecretInput {
  providerId: string
  apiKey: string
}

export interface AiIndexRequest {
  providerId?: string | null
  fullRebuild: boolean
  limit?: number | null
}

export interface AiIndexReport {
  providerId: string
  model: string
  indexedItems: number
  updatedItems: number
  skippedItems: number
  removedItems: number
  lastIndexedAt: string
  notes: string[]
}

export interface AiSearchRequest {
  query: string
  profileId?: string | null
  domain?: string | null
  limit?: number | null
}

export interface AiSearchResultItem {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  domain: string
  visitedAt: string
  score: number
  matchReason: string
}

export interface AiSearchResponse {
  total: number
  providerId: string
  model: string
  items: AiSearchResultItem[]
  notes: string[]
}

export interface AiAssistantRequest {
  question: string
  profileId?: string | null
  domain?: string | null
}

export interface AiAssistantCitation {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  visitedAt: string
  score: number
}

export interface AiAssistantResponse {
  answer: string
  providerId: string
  embeddingProviderId: string
  citations: AiAssistantCitation[]
  notes: string[]
}

export interface AiIntegrationPreview {
  mcpCommand: string
  manualSteps: string[]
  generatedFiles: ScheduleGeneratedFile[]
  warnings: string[]
}
