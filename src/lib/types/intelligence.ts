import type { ScheduleGeneratedFile } from './schedule'

export interface EnrichmentPluginState {
  id: string
  enabled: boolean
  version: string
}

export interface EnrichmentSettings {
  plugins: EnrichmentPluginState[]
}

export interface DeterministicModuleState {
  id: string
  enabled: boolean
  version: string
}

export interface DeterministicSettings {
  modules: DeterministicModuleState[]
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

export interface EnrichmentPluginPreference {
  pluginId: string
  enabled: boolean
}

export interface AiSettings {
  enabled: boolean
  assistantEnabled: boolean
  semanticIndexEnabled: boolean
  mcpEnabled: boolean
  skillEnabled: boolean
  autoIndexAfterBackup: boolean
  jobQueuePaused: boolean
  jobQueueConcurrency: number
  enrichmentEnabled: boolean
  enrichmentPlugins: EnrichmentPluginPreference[]
  llmProviderId?: string | null
  embeddingProviderId?: string | null
  retrievalTopK: number
  assistantSystemPrompt: string
  llmProviders: AiProviderConfig[]
  embeddingProviders: AiProviderConfig[]
}
export interface AiIndexStatus {
  enabled: boolean
  assistantEnabled: boolean
  mcpEnabled: boolean
  skillEnabled: boolean
  state: string
  ready: boolean
  indexedItems: number
  lastIndexedAt?: string | null
  llmProviderId?: string | null
  embeddingProviderId?: string | null
  queuePaused: boolean
  queueConcurrency: number
  queuedJobs: number
  runningJobs: number
  failedJobs: number
  recentJobs: AiQueueJob[]
  semanticSidecarBytes: number
  semanticMirrorBytes: number
  estimatedEmbeddingTokens: number
  warning?: string | null
}

export interface AiProviderCapabilityReport {
  supportsChat: boolean
  supportsEmbeddings: boolean
  supportsStreaming: boolean
  supportsToolUse: boolean
  supportsStructuredOutput: boolean
}

export interface AiProviderConnectionTestRequest {
  providerId: string
  purpose: AiProviderPurpose
}

export interface AiProviderConnectionTestReport {
  providerId: string
  purpose: string
  model: string
  ok: boolean
  latencyMs: number
  capabilities: AiProviderCapabilityReport
  errorCode?: string | null
  actionHint?: string | null
  retryHint?: string | null
  warnings: string[]
  message: string
}

export interface AiQueueJob {
  id: number
  jobType: string
  state: string
  priority: number
  attempt: number
  maxAttempts: number
  runId?: number | null
  summary?: string | null
  queuedAt: string
  availableAt: string
  startedAt?: string | null
  finishedAt?: string | null
  heartbeatAt?: string | null
  errorCode?: string | null
  errorMessage?: string | null
}

export interface AiQueueStatus {
  paused: boolean
  concurrency: number
  queued: number
  running: number
  failed: number
  recentJobs: AiQueueJob[]
}

export interface InsightStatus {
  ready: boolean
  lastRunAt?: string | null
  runs: number
  cards: number
  topics: number
  threads: number
  queryGroups: number
  referencePages: number
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

export interface InsightQueryGroupSummary {
  queryGroupId: string
  profileId: string
  threadId?: string | null
  title: string
  rootQuery: string
  latestQuery: string
  firstSeenAt: string
  lastSeenAt: string
  visitCount: number
  burstCount: number
  stepCount: number
  confidence: number
  evidenceTier: string
  chromiumEnhanced: boolean
  steps: string[]
  stages: string[]
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
  queryGroupCount: number
  reopenCount: number
  openLoopScore: number
  confidence: number
  evidenceTier: string
  dominantTopicId?: string | null
  chromiumEnhanced: boolean
  evidence: InsightEvidenceItem[]
}

export interface InsightThreadDetail {
  summary: InsightThreadSummary
  queryGroups: InsightQueryGroupSummary[]
  visits: InsightEvidenceItem[]
}

export interface InsightQueryLadder {
  queryGroupId?: string | null
  rootTerm: string
  profileId: string
  steps: string[]
  stages: string[]
  count: number
  confidence: number
  evidenceTier: string
  chromiumOnly: boolean
}

export interface InsightReferencePageSummary {
  referencePageId: string
  profileId?: string | null
  url: string
  title?: string | null
  domain: string
  firstSeenAt: string
  lastSeenAt: string
  revisitCount: number
  crossDayRevisits: number
  queryGroupCount: number
  threadCount: number
  score: number
  evidenceTier: string
  evidence: InsightEvidenceItem[]
}

export interface InsightSourceEffectivenessSummary {
  sourceId: string
  profileId?: string | null
  domain: string
  sourceRole: string
  queryGroupCount: number
  threadCount: number
  stableLandingCount: number
  referencePageCount: number
  reopenSupportCount: number
  effectivenessScore: number
  evidenceTier: string
  evidence: InsightEvidenceItem[]
}

export interface InsightTemplateSummary {
  summaryId: string
  kind: string
  title: string
  body: string
  confidence: number
  profileId?: string | null
  evidence: InsightEvidenceItem[]
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

export interface InsightDomainStat {
  domain: string
  visitCount: number
}

export interface InsightCanonicalSummary {
  windowVisitCount: number
  windowUniqueDomains: number
  onThisDay: InsightEvidenceItem[]
  topDomains: InsightDomainStat[]
}

export interface InsightSnapshot {
  generatedAt: string
  windowDays: number
  profileId?: string | null
  status: InsightStatus
  cards: InsightCard[]
  queryGroups: InsightQueryGroupSummary[]
  topics: InsightTopicSummary[]
  threads: InsightThreadSummary[]
  queryLadders: InsightQueryLadder[]
  referencePages: InsightReferencePageSummary[]
  sourceEffectiveness: InsightSourceEffectivenessSummary[]
  templateSummaries: InsightTemplateSummary[]
  workflowMap: InsightWorkflowMap
  profileFacets: InsightProfileFacet[]
  canonical: InsightCanonicalSummary
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
  queryGroupCount: number
  topicCount: number
  threadCount: number
  referencePageCount: number
  sourceCount: number
  templateSummaryCount: number
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

export interface IntelligenceQueueStatus {
  queued: number
  running: number
  succeeded: number
  failed: number
  cancelled: number
  lastActivityAt?: string | null
}

export interface EnrichmentPluginStatus {
  pluginId: string
  sourceKind: string
  enabled: boolean
  storedRecords: number
  queuedJobs: number
  runningJobs: number
  failedJobs: number
  lastCompletedAt?: string | null
  lastError?: string | null
}

export interface IntelligenceJobOverview {
  id: number
  jobType: string
  pluginId?: string | null
  state: string
  historyId?: number | null
  profileId?: string | null
  url?: string | null
  title?: string | null
  attempt: number
  createdAt: string
  startedAt?: string | null
  finishedAt?: string | null
  lastError?: string | null
  retryable: boolean
  cancellable: boolean
}

export interface DeterministicModuleRuntimeStatus {
  moduleId: string
  enabled: boolean
  version: string
  status: string
  dependsOn: string[]
  derivedTables: string[]
  lastRunId?: number | null
  lastBuiltAt?: string | null
  lastInvalidatedAt?: string | null
  staleReason?: string | null
  notes: string[]
}

export interface IntelligenceRuntimeSnapshot {
  queue: IntelligenceQueueStatus
  plugins: EnrichmentPluginStatus[]
  modules: DeterministicModuleRuntimeStatus[]
  recentJobs: IntelligenceJobOverview[]
  notes: string[]
}
export interface ClearDerivedIntelligenceReport {
  clearedEnrichmentRows: number
  clearedFeatureRows: number
  clearedBurstRows: number
  clearedQueryGroupRows: number
  clearedTopicRows: number
  clearedThreadRows: number
  clearedReferencePageRows: number
  clearedSourceRows: number
  clearedModuleRows: number
  clearedCardRows: number
  clearedRunRows: number
  notes: string[]
}

export interface AiProviderSecretInput {
  providerId: string
  apiKey: string
}

export interface AiIndexRequest {
  providerId?: string | null
  fullRebuild: boolean
  clearOnly: boolean
  limit?: number | null
}

export interface AiIndexReport {
  jobId?: number | null
  runId?: number | null
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
  cursor?: string | null
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
  nextCursor?: string | null
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
  state: string
  answer: string
  jobId?: number | null
  runId?: number | null
  providerId: string
  embeddingProviderId: string
  citations: AiAssistantCitation[]
  notes: string[]
}

export interface AiIntegrationPreview {
  mcpCommand: string
  consentSummary: string
  manualSteps: string[]
  capabilityNotes: string[]
  scopeBoundary: string[]
  auditTrace: string[]
  generatedFiles: ScheduleGeneratedFile[]
  warnings: string[]
}
