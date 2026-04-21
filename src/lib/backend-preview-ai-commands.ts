/**
 * @file backend-preview-ai-commands.ts
 * @description Browser-preview AI command owner for extracted intelligence, queue, assistant, and integration preview behavior.
 * @module lib/backend-preview-ai-commands
 *
 * ## Responsibilities
 * - Handle the preview-only AI and intelligence-runtime commands that currently live inside `backend.ts`.
 * - Keep queue, assistant, provider-key, and integration-preview payloads aligned with the existing browser-preview fixture contract.
 * - Reuse the shared preview command sentinel so `backend.ts` can delegate without changing the owned command behavior.
 *
 * ## Not responsible for
 * - Shell/bootstrap commands, archive workflow commands, or non-AI preview surfaces.
 * - Choosing when browser preview is active; the transport split still belongs to `backend.ts`.
 * - Changing AI payload shape, strings, or mock mutation semantics beyond the current `backend.ts` contract.
 *
 * ## Dependencies
 * - Depends on the shared preview command sentinel contract from `./backend-preview-command-result`.
 * - Reuses preview queue/runtime state helpers from `./backend-preview-state`.
 * - Reuses the lexical fallback search helper from `./backend-preview-search`.
 *
 * ## Performance notes
 * - These handlers run synchronously against the in-memory preview fixture, so they should stay bounded and avoid extra cloning beyond returned payloads.
 * - Queue/runtime recomputation remains delegated to the existing shared preview state helpers to avoid drift across consumers.
 */

import {
  PREVIEW_COMMAND_UNHANDLED,
  type PreviewCommandResult,
} from './backend-preview-command-result'
import type { MockBackendState } from './backend-preview-state'
import {
  buildMockQueueStatus,
  syncMockAiStatus,
  syncMockIntelligenceRuntime,
} from './backend-preview-state'
import { paginateMockAiSearch } from './backend-preview-search'
import type {
  AiProviderConnectionTestRequest,
  AiProviderSecretInput,
  AiSearchRequest,
} from './types'

/**
 * Routes the preview commands that belong to the AI/runtime fixture surface and nothing else.
 *
 * This keeps the AI preview contract isolated from shell and workflow extraction work, while preserving
 * the exact browser-preview behavior that `backend.ts` currently exposes for intelligence runtime,
 * queue operations, provider secrets, assistant replies, and integration review payloads.
 */
export function handlePreviewAiCommand<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  state: MockBackendState,
): PreviewCommandResult<T> {
  switch (command) {
    case 'load_intelligence_runtime':
      return structuredClone(state.intelligenceRuntime) as T
    case 'retry_intelligence_job': {
      const jobId = Number(args?.jobId ?? 0)
      state.intelligenceRuntime.recentJobs =
        state.intelligenceRuntime.recentJobs.map((job) =>
          job.id === jobId && job.retryable
            ? {
                ...job,
                state: 'queued',
                finishedAt: null,
                updatedAt: new Date().toISOString(),
                heartbeatAt: null,
                progressLabel: null,
                progressDetail: null,
                progressCurrent: null,
                progressTotal: null,
                progressPercent: null,
                lastError: null,
                retryable: false,
                cancellable: true,
              }
            : job,
        )
      syncMockIntelligenceRuntime(state)
      return structuredClone(state.intelligenceRuntime) as T
    }
    case 'cancel_intelligence_job': {
      const jobId = Number(args?.jobId ?? 0)
      state.intelligenceRuntime.recentJobs =
        state.intelligenceRuntime.recentJobs.map((job) =>
          job.id === jobId && job.cancellable
            ? {
                ...job,
                state: 'cancelled',
                finishedAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                heartbeatAt: null,
                lastError: null,
                retryable: true,
                cancellable: false,
              }
            : job,
        )
      syncMockIntelligenceRuntime(state)
      return structuredClone(state.intelligenceRuntime) as T
    }
    case 'store_ai_provider_api_key': {
      const providerId = (args?.input as AiProviderSecretInput | undefined)
        ?.providerId
      state.snapshot.config.ai.llmProviders =
        state.snapshot.config.ai.llmProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: true }
            : provider,
        )
      state.snapshot.config.ai.embeddingProviders =
        state.snapshot.config.ai.embeddingProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: true }
            : provider,
        )
      return structuredClone(state.snapshot) as T
    }
    case 'clear_ai_provider_api_key': {
      const providerId = args?.providerId as string | undefined
      state.snapshot.config.ai.llmProviders =
        state.snapshot.config.ai.llmProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: false }
            : provider,
        )
      state.snapshot.config.ai.embeddingProviders =
        state.snapshot.config.ai.embeddingProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: false }
            : provider,
        )
      return structuredClone(state.snapshot) as T
    }
    case 'test_ai_provider_connection':
      return {
        providerId:
          (args?.request as AiProviderConnectionTestRequest | undefined)
            ?.providerId ?? 'preview-provider',
        purpose:
          (args?.request as AiProviderConnectionTestRequest | undefined)
            ?.purpose ?? 'embedding',
        model: 'preview-model',
        ok: true,
        latencyMs: 24,
        capabilities: {
          supportsChat: true,
          supportsEmbeddings: true,
          supportsStreaming: true,
          supportsToolUse: true,
          supportsStructuredOutput: true,
        },
        warnings: [],
        message: 'Browser preview mode fakes a successful provider probe.',
      } as T
    case 'load_ai_queue_status':
      syncMockAiStatus(state)
      return buildMockQueueStatus(state) as T
    case 'run_ai_queue_jobs':
      state.queueJobs = state.queueJobs.map((job) =>
        job.state === 'queued'
          ? {
              ...job,
              state: 'succeeded',
              attempt: job.attempt + 1,
              runId: 42,
              finishedAt: new Date().toISOString(),
              summary: 'Preview queue drained this job.',
            }
          : job,
      )
      syncMockAiStatus(state)
      return buildMockQueueStatus(state) as T
    case 'replay_ai_job': {
      const jobId = args?.jobId as number
      state.queueJobs = state.queueJobs.map((job) =>
        job.id === jobId
          ? {
              ...job,
              state: state.snapshot.config.ai.jobQueuePaused
                ? 'paused'
                : 'queued',
              attempt: 0,
              runId: null,
              startedAt: null,
              finishedAt: null,
              heartbeatAt: null,
              errorCode: null,
              errorMessage: null,
            }
          : job,
      )
      syncMockAiStatus(state)
      return structuredClone(
        state.queueJobs.find((job) => job.id === jobId),
      ) as T
    }
    case 'cancel_ai_job': {
      const jobId = args?.jobId as number
      state.queueJobs = state.queueJobs.map((job) =>
        job.id === jobId
          ? {
              ...job,
              state: 'cancelled',
              finishedAt: new Date().toISOString(),
            }
          : job,
      )
      syncMockAiStatus(state)
      return structuredClone(
        state.queueJobs.find((job) => job.id === jobId),
      ) as T
    }
    case 'build_ai_index': {
      const buildJobId = state.nextAiJobId++
      state.queueJobs = [
        {
          id: buildJobId,
          jobType: 'index-build',
          state: 'succeeded',
          priority: 70,
          attempt: 1,
          maxAttempts: 3,
          runId: 31,
          summary: 'Browser preview finished a static index build.',
          queuedAt: new Date().toISOString(),
          availableAt: new Date().toISOString(),
          startedAt: new Date().toISOString(),
          finishedAt: new Date().toISOString(),
          heartbeatAt: new Date().toISOString(),
          errorCode: null,
          errorMessage: null,
        },
        ...state.queueJobs,
      ]
      state.snapshot.aiStatus = {
        ...state.snapshot.aiStatus,
        enabled: true,
        assistantEnabled: true,
        state: 'ready',
        ready: true,
        indexedItems: 2,
        lastIndexedAt: new Date().toISOString(),
        embeddingProviderId: 'mock-embedding',
        semanticSidecarBytes: 196_608,
        semanticMetadataBytes: 24_576,
        estimatedEmbeddingTokens: 1_024,
      }
      syncMockAiStatus(state)
      return {
        jobId: buildJobId,
        runId: 31,
        providerId: 'mock-embedding',
        model: 'text-embedding-3-large',
        indexedItems: 2,
        updatedItems: 0,
        skippedItems: 0,
        removedItems: 0,
        lastIndexedAt: new Date().toISOString(),
        notes: ['Browser preview mode uses a static AI index fixture.'],
      } as T
    }
    case 'search_ai_history':
      return paginateMockAiSearch(
        state,
        args?.request as AiSearchRequest | undefined,
      ) as T
    case 'ask_ai_assistant': {
      const assistantJobId = state.nextAiJobId++
      state.queueJobs = [
        {
          id: assistantJobId,
          jobType: 'assistant',
          state: 'succeeded',
          priority: 100,
          attempt: 1,
          maxAttempts: 1,
          runId: 32,
          summary: 'Browser preview answered a static assistant request.',
          queuedAt: new Date().toISOString(),
          availableAt: new Date().toISOString(),
          startedAt: new Date().toISOString(),
          finishedAt: new Date().toISOString(),
          heartbeatAt: new Date().toISOString(),
          errorCode: null,
          errorMessage: null,
        },
        ...state.queueJobs,
      ]
      syncMockAiStatus(state)
      return {
        state: 'completed',
        answer:
          'Browser preview mode can show the assistant layout, but real LLM answers only run in the desktop app.',
        jobId: assistantJobId,
        runId: 32,
        providerId: 'preview-llm',
        embeddingProviderId: 'lexical-fallback',
        citations: state.history.items.map((item) => ({
          historyId: item.id,
          profileId: item.profileId,
          url: item.url,
          title: item.title,
          visitedAt: item.visitedAt,
          score: 0.8,
        })),
        notes: ['Open the desktop build to run real agentic history analysis.'],
      } as T
    }
    case 'load_ai_assistant_job':
      return {
        state: 'completed',
        answer:
          'Browser preview mode loads a deterministic queued assistant reply.',
        jobId: args?.jobId as number,
        runId: 32,
        providerId: 'preview-llm',
        embeddingProviderId: 'lexical-fallback',
        citations: state.history.items.map((item) => ({
          historyId: item.id,
          profileId: item.profileId,
          url: item.url,
          title: item.title,
          visitedAt: item.visitedAt,
          score: 0.78,
        })),
        notes: [
          'Queued assistant replies use preview fixtures in browser mode.',
        ],
      } as T
    case 'preview_ai_integrations':
      return {
        mcpCommand: '/Applications/PathKeep.app --worker mcp-server',
        consentSummary:
          'External AI integrations stay local-first and only start after the user enables them in Settings.',
        manualSteps: [
          'Enable MCP or Skill integration in Settings first.',
          'Store the database key in the native keyring if the archive is encrypted.',
          'Copy the generated MCP JSON into your MCP client configuration.',
        ],
        capabilityNotes: [
          'MCP server toggle is currently disabled in saved Settings.',
          'Skill integration toggle is currently disabled in saved Settings.',
          'No embedding provider is selected right now, so external tools fall back to lexical recall only.',
        ],
        scopeBoundary: [
          'Only visible archive facts are returned to external tools.',
          'If App Lock re-locks the session, MCP search returns a locked refusal.',
        ],
        auditTrace: [
          'Each MCP search writes a dedicated run-ledger entry.',
          'Assistant and semantic-index work keep distinct run types.',
        ],
        generatedFiles: [
          {
            relativePath: 'integrations/pathkeep-mcp.json',
            absolutePath:
              '~/Library/Application Support/PathKeep/integrations/pathkeep-mcp.json',
            purpose: 'PathKeep MCP client snippet',
            contents: '{\n  "mcpServers": {}\n}',
          },
          {
            relativePath: 'integrations/codex-pathkeep-skill/SKILL.md',
            absolutePath:
              '~/Library/Application Support/PathKeep/integrations/codex-pathkeep-skill/SKILL.md',
            purpose: 'Codex skill starter',
            contents: '# PathKeep Search\n',
          },
        ],
        warnings: [],
      } as T
    default:
      return PREVIEW_COMMAND_UNHANDLED
  }
}
