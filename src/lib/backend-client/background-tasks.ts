/**
 * Typed client for the run ledger's part of the background tasks page:
 * browsing history's backups and imports, the Insights and search-index
 * queues' jobs, and the work done to the whole archive (health check
 * repairs, encryption changes, restores, cleanups), in the same four groups
 * as conversation backups (`list_background_tasks` with `kind: 'browsing'`).
 * With browsing history off the same request answers the whole-archive
 * rows only.
 *
 * Responsible for: the command, its request envelope, the row's wire
 * shape, dismissing a row from "Needs a look", and the event that says
 * these rows changed. Not responsible for: caching
 * (`src/lib/queries/background-tasks.ts`), running a queue command (each
 * names an existing one, called through `intelligence.ts`), or any copy.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md` (Agent
 * viewer, export and shell: the browsing section; Tasks, first-run choices
 * and export control) and `vault-worker/src/data_kinds/backups/browsing_tasks.rs`.
 * Times are UTC epoch milliseconds.
 */
import type { CursorPage, TaskGroup } from './agent-backup'
import { call } from './shared'

/**
 * A committed write changed the ledger's rows (a run or a queue job began,
 * moved on or ended; one event per transaction). `taskId` is null: every
 * list of them is out of date, whole-archive rows included.
 */
export const BACKGROUND_TASKS_CHANGED_EVENT =
  'pathkeep://background-tasks-changed'
export interface BackgroundTasksChangedEvent {
  kind: 'browsing'
  taskId: string | null
}

/** Which ledger a browsing row comes from: a run, an Insights job, a search-index job. */
export type BrowsingTaskSource = 'run' | 'intelligence' | 'index'

/**
 * What can be done to a row: a native queue command, called with
 * `{ jobId }`, or `dismiss`, which takes a failed or partial row out of
 * "Needs a look" (`control_background_task`; its history stays). Only
 * these are offered, and only while the row's state allows them.
 */
export type BrowsingTaskCommand =
  | 'retry_intelligence_job'
  | 'cancel_intelligence_job'
  | 'pause_ai_job'
  | 'cancel_ai_job'
  | 'replay_ai_job'
  | 'dismiss'

/**
 * One row of the ledger. `operation` is the run type (`backup`, `import`,
 * `rekey`, `doctor`, …) or the job type (`full-rebuild`, `index-build`,
 * …); `state` is the ledger's, with `succeeded` for a finished one.
 * `kind` is `archive` for work done to the whole archive, which belongs to
 * no space.
 */
export interface BrowsingTask {
  /** `browsing:<source>/<jobId>`, or `archive:run/<jobId>`. */
  taskId: string
  jobId: number
  kind: 'browsing' | 'archive'
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
  const source = /^(?:browsing|archive):([a-z]+)\//.exec(task.taskId)?.[1]
  return (source ?? 'run') as BrowsingTaskSource
}

/** A ledger row among the rows `get_task_summary` mixes together. */
export function isLedgerTask(row: { kind?: string }): row is BrowsingTask {
  return row.kind === 'browsing' || row.kind === 'archive'
}

export const backgroundTasksClient = {
  /** One group of ledger work, newest first; pass `nextCursor` back for the next page. */
  browsing: (group: TaskGroup, limit: number, cursor?: string) =>
    call<CursorPage<BrowsingTask>>('list_background_tasks', {
      request: {
        group,
        kind: 'browsing',
        limit,
        ...(cursor ? { cursor } : {}),
      },
    }),
  /** Takes a failed or partial row out of "Needs a look"; it stays in its history. */
  dismiss: (taskId: string) =>
    call<{ taskId: string; dismissed: true }>('control_background_task', {
      request: { taskId, action: 'dismiss' },
    }),
}
