/**
 * Typed client for browsing history's part of the background tasks page:
 * its backups and imports from the run ledger, and the Insights and
 * search-index queues' jobs, in the same four groups as conversation
 * backups (`list_background_tasks` with `kind: 'browsing'`).
 *
 * Responsible for: the command, its request envelope and the row's wire
 * shape. Not responsible for: caching (`src/lib/queries/background-tasks.ts`),
 * running a row's commands (each names an existing queue command, called
 * through `intelligence.ts`), or any copy.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md` (Agent
 * viewer, export and shell: the browsing section) and
 * `vault-worker/src/data_kinds/backups/browsing_tasks.rs`. Times are UTC
 * epoch milliseconds; an empty answer means browsing history is off.
 */
import type { CursorPage, TaskGroup } from './agent-backup'
import { call } from './shared'

/** Which ledger a browsing row comes from: a run, an Insights job, a search-index job. */
export type BrowsingTaskSource = 'run' | 'intelligence' | 'index'

/**
 * The native command that does something to a row, called with
 * `{ jobId }`. Only these are offered, and only while the job's state
 * allows them.
 */
export type BrowsingTaskCommand =
  | 'retry_intelligence_job'
  | 'cancel_intelligence_job'
  | 'pause_ai_job'
  | 'cancel_ai_job'
  | 'replay_ai_job'

/**
 * One browsing row. `operation` is the run type (`backup`, `import`, …)
 * or the job type (`full-rebuild`, `index-build`, …); `state` is the
 * ledger's, with `succeeded` for a finished one.
 */
export interface BrowsingTask {
  /** `browsing:<source>/<jobId>`. */
  taskId: string
  jobId: number
  kind: 'browsing'
  operation: string
  state: string
  /** When it was added (waiting, running) or when it ended. */
  at: number | null
  endedAt: number | null
  /** Done and total where the job counts them; `null` before it reports. */
  progress: { done: number; total: number | null } | null
  attention: boolean
  /** A run's reference (`browsing:<origin>/<id>`); `null` for a queue job. */
  runRef: string | null
  commands: BrowsingTaskCommand[]
}

/** Which ledger a row came from, read from its id. */
export function browsingTaskSource(task: Pick<BrowsingTask, 'taskId'>) {
  const source = /^browsing:([a-z]+)\//.exec(task.taskId)?.[1]
  return (source ?? 'run') as BrowsingTaskSource
}

export const backgroundTasksClient = {
  /** One group of browsing work, newest first; pass `nextCursor` back for the next page. */
  browsing: (group: TaskGroup, limit: number, cursor?: string) =>
    call<CursorPage<BrowsingTask>>('list_background_tasks', {
      request: {
        group,
        kind: 'browsing',
        limit,
        ...(cursor ? { cursor } : {}),
      },
    }),
}
