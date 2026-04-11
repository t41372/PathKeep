/**
 * This module defines typed front-end contracts for AI providers, deterministic insights, queue state, and runtime review surfaces.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `EnrichmentPluginState`
 * - `EnrichmentSettings`
 * - `DeterministicModuleState`
 * - `DeterministicSettings`
 * - `AiRequestFormat`
 * - `AiProviderPurpose`
 * - `AiProviderConfig`
 * - `EnrichmentPluginPreference`
 * - `AiSettings`
 * - `AiIndexStatus`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

import type { ScheduleGeneratedFile } from './schedule'

/**
 * Captures the state shape used by `EnrichmentPlugin`.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface EnrichmentPluginState {
  id: string
  enabled: boolean
  version: string
}

/**
 * Defines the typed shape for enrichment settings.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface EnrichmentSettings {
  plugins: EnrichmentPluginState[]
}

/**
 * Captures the state shape used by `DeterministicModule`.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface DeterministicModuleState {
  id: string
  enabled: boolean
  version: string
}

/**
 * Defines the typed shape for deterministic settings.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface DeterministicSettings {
  modules: DeterministicModuleState[]
}

/**
 * Defines the type-level contract for ai request format.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type AiRequestFormat =
  | 'openai'
  | 'anthropic'
  | 'google'
  | 'ollama'
  | 'lm-studio'

/**
 * Defines the type-level contract for ai provider purpose.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type AiProviderPurpose = 'llm' | 'embedding'

/**
 * Represents persisted configuration for ai provider.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for enrichment plugin preference.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface EnrichmentPluginPreference {
  pluginId: string
  enabled: boolean
}

/**
 * Defines the typed shape for ai settings.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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
/**
 * Represents a read model or status snapshot for ai index.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderCapabilityReport {
  supportsChat: boolean
  supportsEmbeddings: boolean
  supportsStreaming: boolean
  supportsToolUse: boolean
  supportsStructuredOutput: boolean
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderConnectionTestRequest {
  providerId: string
  purpose: AiProviderPurpose
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for ai queue job.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a read model or status snapshot for ai queue.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiQueueStatus {
  paused: boolean
  concurrency: number
  queued: number
  running: number
  failed: number
  recentJobs: AiQueueJob[]
}

/**
 * Represents a read model or status snapshot for insight.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for insight evidence item.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightEvidenceItem {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  visitedAt: string
  note?: string | null
}

/**
 * Defines the typed shape for insight card.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a condensed summary for insight query group.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a condensed summary for insight topic.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a condensed summary for insight thread.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents the detailed view model for insight thread.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightThreadDetail {
  summary: InsightThreadSummary
  queryGroups: InsightQueryGroupSummary[]
  visits: InsightEvidenceItem[]
}

/**
 * Defines the typed shape for insight query ladder.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a condensed summary for insight reference page.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a condensed summary for insight source effectiveness.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a condensed summary for insight template.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightTemplateSummary {
  summaryId: string
  kind: string
  title: string
  body: string
  confidence: number
  profileId?: string | null
  evidence: InsightEvidenceItem[]
}

/**
 * Defines the typed shape for insight workflow role.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightWorkflowRole {
  role: string
  count: number
}

/**
 * Defines the typed shape for insight workflow edge.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightWorkflowEdge {
  fromRole: string
  toRole: string
  count: number
}

/**
 * Defines the typed shape for insight workflow map.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightWorkflowMap {
  profileId?: string | null
  roles: InsightWorkflowRole[]
  edges: InsightWorkflowEdge[]
  chromiumEnhanced: boolean
}

/**
 * Defines the typed shape for insight profile facet.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightProfileFacet {
  key: string
  label: string
  value: string
  confidence: number
  evidence: InsightEvidenceItem[]
}

/**
 * Defines the typed shape for insight domain stat.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightDomainStat {
  domain: string
  visitCount: number
}

/**
 * Represents a condensed summary for insight canonical.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightCanonicalSummary {
  windowVisitCount: number
  windowUniqueDomains: number
  onThisDay: InsightEvidenceItem[]
  topDomains: InsightDomainStat[]
}

/**
 * Defines the typed shape for insight snapshot.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RunInsightsRequest {
  profileId?: string | null
  windowDays?: number | null
  fullRebuild: boolean
  limit?: number | null
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ExplainInsightRequest {
  insightId: string
  insightKind: string
  profileId?: string | null
  windowDays?: number | null
}

/**
 * Defines the typed shape for insight explanation.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface InsightExplanation {
  explanation: string
  usedLlm: boolean
  citations: InsightEvidenceItem[]
  notes: string[]
}

/**
 * Represents a read model or status snapshot for intelligence queue.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface IntelligenceQueueStatus {
  queued: number
  running: number
  succeeded: number
  failed: number
  cancelled: number
  lastActivityAt?: string | null
}

/**
 * Represents a read model or status snapshot for enrichment plugin.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for intelligence job overview.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents a read model or status snapshot for deterministic module runtime.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for intelligence runtime snapshot.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface IntelligenceRuntimeSnapshot {
  queue: IntelligenceQueueStatus
  plugins: EnrichmentPluginStatus[]
  modules: DeterministicModuleRuntimeStatus[]
  recentJobs: IntelligenceJobOverview[]
  notes: string[]
}
/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for ai provider secret input.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderSecretInput {
  providerId: string
  apiKey: string
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiIndexRequest {
  providerId?: string | null
  fullRebuild: boolean
  clearOnly: boolean
  limit?: number | null
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiSearchRequest {
  query: string
  profileId?: string | null
  domain?: string | null
  limit?: number | null
  cursor?: string | null
}

/**
 * Defines the typed shape for ai search result item.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Describes a response payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiSearchResponse {
  total: number
  providerId: string
  model: string
  items: AiSearchResultItem[]
  notes: string[]
  nextCursor?: string | null
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiAssistantRequest {
  question: string
  profileId?: string | null
  domain?: string | null
}

/**
 * Defines the typed shape for ai assistant citation.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiAssistantCitation {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  visitedAt: string
  score: number
}

/**
 * Describes a response payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Represents the preview payload shown before a write or high-risk action happens.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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
