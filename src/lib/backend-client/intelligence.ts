/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `intelligenceClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import type {
  AgentConversationDetail,
  AgentConversationListResponse,
  AgentConversationSummary,
  AiChatCancelResult,
  AiChatSendAck,
  AiChatContext,
  AiChatContextRequest,
  AiChatSendRequest,
  AiIndexReport,
  AiIntegrationPreview,
  AiProviderConnectionTestReport,
  AiProviderConnectionTestRequest,
  AiProviderModel,
  AiProviderModelsRequest,
  AiProviderSecretInput,
  AiQueueJob,
  AiQueueStatus,
  AiSearchRequest,
  AiSearchResponse,
  AppSnapshot,
  BuildAiIndexRequest,
  ClearDerivedIntelligenceReport,
  ConversationExportOutcome,
  DeleteAgentConversationResult,
  ExportAgentConversationRequest,
  IntelligenceJobOverview,
  IntelligenceRuntimeSnapshot,
  JobPage,
  ListAgentConversationsRequest,
  ListJobsRequest,
  McpQueryRecord,
  ModelDownloadState,
  ModelDownloadTransfer,
  ReembedEstimate,
  ReembedScope,
  RenameAgentConversationRequest,
  SaveAgentConversationRequest,
} from '../types'
import type {
  CoreIntelligenceQueueReport,
  CoreIntelligencePrimaryOverview,
  SearchEngineRule,
  SearchEngineRuleInput,
  CoreIntelligenceSecondaryOverview,
  DayInsights,
} from '../core-intelligence/types'
import type { FreeSemanticPreview } from './agent-semantic'
import { call } from './shared'

/**
 * Exposes the focused client surface for intelligence commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export const intelligenceClient = {
  storeProviderApiKey: (input: AiProviderSecretInput) =>
    call<AppSnapshot>('store_ai_provider_api_key', { input }),
  clearProviderApiKey: (providerId: string) =>
    call<AppSnapshot>('clear_ai_provider_api_key', { providerId }),
  testProviderConnection: (request: AiProviderConnectionTestRequest) =>
    call<AiProviderConnectionTestReport>('test_ai_provider_connection', {
      request,
    }),
  listProviderModels: (request: AiProviderModelsRequest) =>
    call<AiProviderModel[]>('list_ai_provider_models', { request }),
  getQueueStatus: () => call<AiQueueStatus>('load_ai_queue_status'),
  /** What outside apps asked through the MCP server, newest first. */
  listMcpQueries: (cursor: number | null, limit: number) =>
    call<JobPage<McpQueryRecord>>('load_mcp_queries', { cursor, limit }),
  /** Every search-index and assistant job, a page at a time. */
  listQueueJobs: (request: ListJobsRequest) =>
    call<JobPage<AiQueueJob>>('list_ai_jobs', { request }),
  runQueueJobs: (maxJobs?: number) =>
    call<AiQueueStatus>('run_ai_queue_jobs', { maxJobs }),
  replayJob: (jobId: number) => call<AiQueueJob>('replay_ai_job', { jobId }),
  cancelJob: (jobId: number) => call<AiQueueJob>('cancel_ai_job', { jobId }),
  /**
   * Pauses an index build where it is; `replayJob` goes on from there. A
   * paused build stays paused across restarts and backups.
   */
  pauseJob: (jobId: number) => call<AiQueueJob>('pause_ai_job', { jobId }),
  /** With `transferId`, the build the built-in model's setup asks for once its download is done. */
  buildIndex: (request: BuildAiIndexRequest) =>
    call<AiIndexReport>('build_ai_index', { request }),
  /**
   * Reads what freeing the browsing semantic index would delete: every
   * model's files. Only while semantic search is off and no index job is
   * queued or running (`task-action-unavailable`, `archive-busy` otherwise).
   * A new preview replaces the one before.
   */
  previewFreeSemanticIndex: () =>
    call<FreeSemanticPreview>('preview_free_semantic_index', { request: {} }),
  /**
   * Deletes exactly what the preview counted; `preview-expired` once
   * anything changed since or five minutes passed. Visits, full-text search
   * and saved page text stay.
   */
  freeSemanticIndex: (previewId: string) =>
    call<{ freedBytes: string; keptBytes: string }>('free_semantic_index', {
      request: { previewId, confirm: true },
    }),
  estimateReembed: (scope: ReembedScope) =>
    call<ReembedEstimate>('estimate_reembed', { scope }),
  searchHistory: (request: AiSearchRequest) =>
    call<AiSearchResponse>('search_ai_history', { request }),
  chatContext: (request: AiChatContextRequest) =>
    call<AiChatContext>('ai_chat_context', { request }),
  sendChat: (request: AiChatSendRequest) =>
    call<AiChatSendAck>('ai_chat_send', { request }),
  cancelChat: (runId: string) =>
    call<AiChatCancelResult>('ai_chat_cancel', { runId }),
  saveConversation: (request: SaveAgentConversationRequest) =>
    call<AgentConversationSummary>('save_ai_conversation', { request }),
  listConversations: (request: ListAgentConversationsRequest = {}) =>
    call<AgentConversationListResponse>('list_ai_conversations', { request }),
  loadConversation: (conversationId: string) =>
    call<AgentConversationDetail | null>('load_ai_conversation', {
      conversationId,
    }),
  deleteConversation: (conversationId: string) =>
    call<DeleteAgentConversationResult>('delete_ai_conversation', {
      conversationId,
    }),
  renameConversation: (request: RenameAgentConversationRequest) =>
    call<AgentConversationSummary | null>('rename_ai_conversation', {
      request,
    }),
  exportConversation: (request: ExportAgentConversationRequest) =>
    call<ConversationExportOutcome>('export_ai_conversation', {
      request,
    }),
  listSearchEngineRules: () =>
    call<SearchEngineRule[]>('list_search_engine_rules'),
  upsertSearchEngineRule: (input: SearchEngineRuleInput) =>
    call<SearchEngineRule[]>('upsert_search_engine_rule', { input }),
  deleteSearchEngineRule: (ruleId: string) =>
    call<SearchEngineRule[]>('delete_search_engine_rule', { ruleId }),
  queueCoreIntelligenceRebuild: (request: {
    profileId?: string | null
    fullRebuild: boolean
    limit?: number | null
  }) =>
    call<CoreIntelligenceQueueReport>('queue_core_intelligence_rebuild', {
      request,
    }),
  /** Counts what `clearDerivedState` would remove, without removing it (the PME preview). */
  previewClearDerivedState: () =>
    call<ClearDerivedIntelligenceReport>('preview_clear_derived_intelligence'),
  clearDerivedState: () =>
    call<ClearDerivedIntelligenceReport>('clear_derived_intelligence'),
  getPrimaryOverview: (request: {
    dateRange: { start: string; end: string }
    profileId?: string | null
  }) =>
    call<CoreIntelligencePrimaryOverview>('get_intelligence_primary_overview', {
      request,
    }),
  getSecondaryOverview: (request: {
    dateRange: { start: string; end: string }
    profileId?: string | null
  }) =>
    call<CoreIntelligenceSecondaryOverview>(
      'get_intelligence_secondary_overview',
      {
        request,
      },
    ),
  getDayInsights: (request: { date: string; profileId?: string | null }) =>
    call<DayInsights>('get_day_insights', {
      request,
    }),
  getRuntime: () =>
    call<IntelligenceRuntimeSnapshot>('load_intelligence_runtime'),
  /** Every Insights job, a page at a time. */
  listRuntimeJobs: (request: ListJobsRequest) =>
    call<JobPage<IntelligenceJobOverview>>('list_intelligence_jobs', {
      request,
    }),
  retryRuntimeJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('retry_intelligence_job', { jobId }),
  cancelRuntimeJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('cancel_intelligence_job', { jobId }),
  previewIntegrations: () =>
    call<AiIntegrationPreview>('preview_ai_integrations'),
  /**
   * Starts the built-in embedding model's download, or joins the one running, and answers with its
   * transfer ID. Progress and the one terminal event (`done`, `error` or `cancelled`) arrive on
   * `pathkeep://model-download-progress` (`lib/ipc/model-download`); subscribe before calling.
   * Refused, with no transfer, when the saved settings do not want the built-in model.
   */
  downloadStaticEmbeddingModel: () =>
    call<ModelDownloadTransfer>('download_static_embedding_model'),
  /**
   * Cancels one model download by its transfer ID; that transfer then ends with `cancelled`.
   * Answers `false` once its publication or a final failure was decided (it then reports its real
   * outcome), and for an unknown or already cancelled ID, so a late click never stops a newer one.
   */
  cancelStaticEmbeddingModelDownload: (transferId: string) =>
    call<boolean>('cancel_ai_embedding_model_download', { transferId }),
  /**
   * The built-in model's running transfer and its counts so far, or null when none runs. Listen to
   * the progress channel first: its end arrives there, possibly before this answer.
   */
  getModelDownloadState: () =>
    call<ModelDownloadState | null>('get_model_download_state'),
  /**
   * Clears the stuck/failed index build job and re-runs the build as a FULL REBUILD: every page is
   * re-embedded from scratch and the vector store + derived planes are wiped first. This is the only
   * recovery that fixes the 0-vector "degraded" case — incremental dedup keys `needs_embedding` on
   * the SQLite embedding rows, not the `.pkvec`, so an incremental pass would skip a torn/empty
   * store and leave it broken. The backend coerces this to a full rebuild regardless of the request,
   * so the explicit `fullRebuild` here just satisfies the wire contract (`AiIndexRequest` has no
   * serde default for its bools). Your archive/history is NOT deleted — only the derived search
   * index is rebuilt. Use when the index state is `failed` or `degraded`. Idempotent.
   */
  resetAiIndexBuild: () =>
    call<void>('reset_ai_index_build', {
      request: { fullRebuild: true, clearOnly: false },
    }),
}
