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
  AiAssistantRequest,
  AiAssistantResponse,
  AiChatCancelResult,
  AiChatSendAck,
  AiChatSendRequest,
  AiIndexReport,
  AiIndexRequest,
  AiIntegrationPreview,
  AiProviderConnectionTestReport,
  AiProviderConnectionTestRequest,
  AiProviderSecretInput,
  AiQueueJob,
  AiQueueStatus,
  AiSearchRequest,
  AiSearchResponse,
  AppSnapshot,
  ClearDerivedIntelligenceReport,
  DeleteAgentConversationResult,
  IntelligenceRuntimeSnapshot,
  ListAgentConversationsRequest,
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
  getQueueStatus: () => call<AiQueueStatus>('load_ai_queue_status'),
  runQueueJobs: (maxJobs?: number) =>
    call<AiQueueStatus>('run_ai_queue_jobs', { maxJobs }),
  replayJob: (jobId: number) => call<AiQueueJob>('replay_ai_job', { jobId }),
  cancelJob: (jobId: number) => call<AiQueueJob>('cancel_ai_job', { jobId }),
  buildIndex: (request: AiIndexRequest) =>
    call<AiIndexReport>('build_ai_index', { request }),
  estimateReembed: (scope: ReembedScope) =>
    call<ReembedEstimate>('estimate_reembed', { scope }),
  searchHistory: (request: AiSearchRequest) =>
    call<AiSearchResponse>('search_ai_history', { request }),
  askAssistant: (request: AiAssistantRequest) =>
    call<AiAssistantResponse>('ask_ai_assistant', { request }),
  getAssistantJob: (jobId: number) =>
    call<AiAssistantResponse>('load_ai_assistant_job', { jobId }),
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
  retryRuntimeJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('retry_intelligence_job', { jobId }),
  cancelRuntimeJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('cancel_intelligence_job', { jobId }),
  previewIntegrations: () =>
    call<AiIntegrationPreview>('preview_ai_integrations'),
  /**
   * Consents to and initiates the download of the built-in static embedding model weights.
   * The download runs in the background; live per-file progress and the terminal done/error
   * arrive on the `pathkeep://model-download-progress` channel (see `lib/ipc/model-download`),
   * and `aiStatus.staticEmbedding.modelDownloaded` flips true on the next poll once present.
   */
  downloadStaticEmbeddingModel: () =>
    call<void>('download_static_embedding_model'),
  /**
   * Requests cancellation of an in-flight in-app embedding model download (static or heavy tier;
   * they share one process-global cancel flag). The cancelled download emits a terminal `error`
   * event on the progress channel. No-op when nothing is downloading.
   */
  cancelStaticEmbeddingModelDownload: () =>
    call<void>('cancel_ai_embedding_model_download'),
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
