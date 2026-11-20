/**
 * Typed client for the Agent conversations' semantic search index: whether
 * it may exist, the generation that serves searches, and the build that
 * makes the next one (`docs/architecture/desktop-command-surface.md`,
 * "Agent conversation AI access").
 *
 * Responsible for: command names, the request envelope, the wire shape and
 * the change event. Not responsible for: caching
 * (`src/lib/queries/agent-semantic.ts`), searching (`agent-search.ts`,
 * `mode: 'semantic'`) or any copy.
 */
import { call } from './shared'

/** A build: running, paused or stopping while active; then ready, cancelled or failed. */
export type SemanticJobState =
  'running' | 'paused' | 'cancelling' | 'ready' | 'cancelled' | 'failed'

export interface SemanticJob {
  jobId: string
  state: SemanticJobState
  /** Passages read in this build; the total is unknown until it publishes. */
  passages: number
  /** Distinct vectors made in this build (repeated text shares one). */
  vectors: number
  /** Why a build failed: an `AiRefusalCode`, or another backend code. */
  error: string | null
}

/** The finished generation searches use. */
export interface SemanticGeneration {
  generation: string
  provider: string
  model: string
  passages: number
  vectors: number
  /** Distinct texts whose vectors came from the generation before. */
  reusedVectors?: number
  /** Unix milliseconds. */
  builtAt: number
}

export interface SemanticIndexStatus {
  /**
   * The switch in Settings › Agent conversations and the AI capability are
   * both on. Off keeps whatever was built on this computer and uses none of it.
   */
  enabled: boolean
  /**
   * Bytes the published index takes on this computer (a decimal string),
   * encryption included; "0" before the first one and after it is freed.
   * Shown while the switch is off: the index stays, unused, until freed.
   */
  keptBytes: string
  serving: SemanticGeneration | null
  /** Conversations changed since the serving generation (or none exists). */
  stale?: boolean
  job: SemanticJob | null
  /** `embedding-provider-unavailable`: no embedding model is chosen in Settings › AI. */
  reason?: string
}

export type SemanticAction = 'build' | 'pause' | 'resume' | 'cancel'

/**
 * The build as the task reads list it beside conversation backups
 * (`operation: "semantic-index"`). It has no run, no start time and no
 * token: its actions go to `control_agent_semantic_index` with `jobId`.
 * Only an active build is listed: running or stopping under Running,
 * paused under Waiting.
 */
export interface SemanticTask {
  taskId: string
  jobId: string
  kind: 'agent-history'
  operation: 'semantic-index'
  state: 'running' | 'paused' | 'cancelling'
  phase: string
  /** `done` is passages read, as a decimal string; no total is known. */
  progress: { done: string; total: null; unit: 'passages' }
  actions: ('pause' | 'resume' | 'cancel')[]
  controlCommand: 'control_agent_semantic_index'
  attention: boolean
  holdsQuit: boolean
}

export function isSemanticTask(task: {
  operation?: string
}): task is SemanticTask {
  return task.operation === 'semantic-index'
}

/** Sent with the build's progress and each change of its state: a `SemanticJob`. */
export const AGENT_SEMANTIC_INDEX_CHANGED_EVENT =
  'pathkeep://agent-semantic-index-changed'

/** A look at what freeing would delete; its id is good for one free, for five minutes. */
export interface FreeSemanticPreview {
  previewId: string
  keptBytes: string
}

export const agentSemanticClient = {
  status: () =>
    call<SemanticIndexStatus>('get_agent_semantic_index', { request: {} }),
  /**
   * `build` starts one (or returns the one running, or `{state:'ready'}`
   * when nothing changed); the others act on the job with that id.
   */
  control: (action: SemanticAction, jobId?: string) =>
    call<SemanticJob | { state: 'ready' }>('control_agent_semantic_index', {
      request: action === 'build' ? { action } : { action, jobId },
    }),
  /**
   * Only while the switch is off and no build runs (`task-action-unavailable`,
   * `archive-busy` otherwise). A new preview replaces the one before.
   */
  previewFree: () =>
    call<FreeSemanticPreview>('preview_free_agent_semantic_index', {
      request: {},
    }),
  /**
   * Deletes the kept index the preview showed; `preview-expired` once the
   * archive or the index changed since. Conversations, full-text search and
   * browsing's index stay.
   */
  free: (previewId: string) =>
    call<{ freedBytes: string; keptBytes: string }>(
      'free_agent_semantic_index',
      { request: { previewId, confirm: true } },
    ),
}
