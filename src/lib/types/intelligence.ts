/**
 * This module defines typed front-end contracts for AI providers, Core Intelligence, queue state, and runtime review surfaces.
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
  /**
   * Internal runtime marker used for diagnostics and storage diffing.
   * The primary UI should not surface this value in normal review chrome.
   */
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
  /**
   * Internal runtime marker used for diagnostics and storage diffing.
   * The primary UI should not surface this value in normal review chrome.
   */
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
  semanticMetadataBytes: number
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
 * Serialized, UI-facing mirror of the in-engine LLM capabilities.
 *
 * Attached to a connection-test report for LLM providers (null for embedding providers). It
 * exposes the exact streaming/tool/structured/cache facts the chat transport relies on, plus
 * the known context window (`maxContextTokens` is null unless the transport reports one).
 */
export interface LlmProviderCapabilityReport {
  toolCall: boolean
  structuredOutput: boolean
  streaming: boolean
  promptCache: boolean
  maxContextTokens?: number | null
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
  llmCapabilities?: LlmProviderCapabilityReport | null
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
 * Represents a read model or status snapshot for Core Intelligence runtime readiness.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface IntelligenceStatus {
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
  updatedAt: string
  heartbeatAt?: string | null
  progressLabel?: string | null
  progressDetail?: string | null
  progressCurrent?: number | null
  progressTotal?: number | null
  progressPercent?: number | null
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
  clearedVisitDerivedFactRows: number
  clearedDailyRollupRows: number
  clearedStructuralRows: number
  clearedRuntimeRows: number
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
 * Conversational role for one streaming chat message (mirrors the backend `LlmRole`).
 */
export type AiChatRole = 'system' | 'user' | 'assistant' | 'tool'

/**
 * One message in a streaming chat request.
 */
export interface AiChatMessage {
  role: AiChatRole
  content: string
}

/**
 * Request payload for `ai_chat_send`. `providerId` defaults to the configured LLM provider when
 * omitted; `temperature`/`maxTokens` override that provider's defaults for this turn only.
 */
export interface AiChatSendRequest {
  providerId?: string | null
  messages: AiChatMessage[]
  temperature?: number | null
  maxTokens?: number | null
}

/**
 * Acknowledgement from `ai_chat_send`: the run id used to subscribe to and cancel the stream.
 */
export interface AiChatSendAck {
  runId: string
}

/**
 * Result of `ai_chat_cancel`: whether a live run with the given id was found and asked to stop.
 */
export interface AiChatCancelResult {
  cancelled: boolean
}

/**
 * One streamed chat chunk delivered over `pathkeep://ai-stream`.
 *
 * The `kind` tag routes each chunk into its UI lane: visible `token` text, model `reasoning`,
 * a requested `toolCall`, the terminal `done` marker, or a terminal `error` with a message.
 */
export type AiChatStreamChunk =
  | { kind: 'token'; text: string }
  | { kind: 'reasoning'; text: string }
  | { kind: 'toolCall'; name: string; arguments: string }
  | { kind: 'done' }
  | { kind: 'error'; message: string }

/**
 * Envelope emitted on `pathkeep://ai-stream` pairing a chunk with its run id.
 */
export interface AiChatStreamEvent {
  runId: string
  chunk: AiChatStreamChunk
}

/**
 * One persisted assistant-chat message (mirrors the backend `AgentMessage` and the in-memory
 * `ChatMessage` shape). `toolCallsJson` is the serialized `AssistantToolCall[]` exactly as it was
 * rendered; persisting it as opaque JSON keeps the store decoupled from the evolving tool schema.
 */
export interface AgentMessage {
  id: string
  /** `'user'` or `'assistant'`. */
  role: string
  content: string
  reasoning?: string | null
  toolCallsJson?: string | null
  /** Terminal turn status (`done` / `error` / `cancelled`); absent for user messages. */
  status?: string | null
}

/**
 * Lightweight conversation row for the chat-history explorer list (no message bodies).
 */
export interface AgentConversationSummary {
  id: string
  title: string
  /** LLM provider id active when saved (display only; never a model id). */
  providerId?: string | null
  createdAt: string
  updatedAt: string
  messageCount: number
}

/**
 * One conversation plus its full (bounded) message transcript. The summary fields are flattened
 * onto this object by the backend (`#[serde(flatten)]`), so it carries both the list metadata and
 * the messages.
 */
export interface AgentConversationDetail extends AgentConversationSummary {
  messages: AgentMessage[]
}

/**
 * Request payload for `save_ai_conversation`: upsert a conversation + replace its messages. When
 * `title` is omitted/blank the backend derives one from the first user message.
 */
export interface SaveAgentConversationRequest {
  id: string
  title?: string | null
  providerId?: string | null
  messages: AgentMessage[]
}

/**
 * Request payload for `list_ai_conversations`: a bounded, newest-first page cap.
 */
export interface ListAgentConversationsRequest {
  limit?: number | null
}

/**
 * Response for `list_ai_conversations`.
 */
export interface AgentConversationListResponse {
  conversations: AgentConversationSummary[]
}

/**
 * Request payload for `rename_ai_conversation`.
 */
export interface RenameAgentConversationRequest {
  id: string
  title: string
}

/**
 * Result of `delete_ai_conversation`: whether a row with the id existed and was removed.
 */
export interface DeleteAgentConversationResult {
  deleted: boolean
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
