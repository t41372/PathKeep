import type {
  AiAssistantRequest,
  AiAssistantResponse,
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
  ExplainInsightRequest,
  InsightExplanation,
  InsightSnapshot,
  InsightThreadDetail,
  IntelligenceRuntimeSnapshot,
  RunInsightsReport,
  RunInsightsRequest,
} from '../types'
import { call } from './shared'

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
  searchHistory: (request: AiSearchRequest) =>
    call<AiSearchResponse>('search_ai_history', { request }),
  askAssistant: (request: AiAssistantRequest) =>
    call<AiAssistantResponse>('ask_ai_assistant', { request }),
  getAssistantJob: (jobId: number) =>
    call<AiAssistantResponse>('load_ai_assistant_job', { jobId }),
  runInsights: (request: RunInsightsRequest) =>
    call<RunInsightsReport>('run_insights_now', { request }),
  clearDerivedState: () =>
    call<ClearDerivedIntelligenceReport>('clear_derived_intelligence'),
  getInsightsSnapshot: (request: RunInsightsRequest) =>
    call<InsightSnapshot>('load_insights', { request }),
  getThreadDetail: (threadId: string) =>
    call<InsightThreadDetail>('load_thread_detail', { threadId }),
  explainInsight: (request: ExplainInsightRequest) =>
    call<InsightExplanation>('explain_insight', { request }),
  getRuntime: () =>
    call<IntelligenceRuntimeSnapshot>('load_intelligence_runtime'),
  retryRuntimeJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('retry_intelligence_job', { jobId }),
  cancelRuntimeJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('cancel_intelligence_job', { jobId }),
  previewIntegrations: () =>
    call<AiIntegrationPreview>('preview_ai_integrations'),
}
