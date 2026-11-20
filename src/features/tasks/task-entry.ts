/**
 * One row of the background tasks page, whichever kind of data its task
 * serves: a conversation backup (with its snapshot-token actions), the
 * conversations' semantic index build (with its job's controls), or a row
 * of the run ledger: browsing history's work (a backup or import, or a
 * queue job, with the command it offers) or work on the whole archive.
 *
 * Responsibilities: the row's sort time in each group; merging the two
 * kinds' pages, each newest first, into one list that never shows a row
 * out of place (an entry is shown only when no unread page of either kind
 * could hold a newer one); and how a browsing row is named and its state
 * put into words. Not responsible for: drawing (`task-row.tsx`) or
 * conversation backups' words (`features/agent-backup/words.ts`).
 */
import type { KindTask, TaskGroup } from '@/lib/backend-client/agent-backup'
import type { SemanticTask } from '@/lib/backend-client/agent-semantic'
import {
  browsingTaskSource,
  type BrowsingTask,
} from '@/lib/backend-client/background-tasks'
import type { Translator, MessageKey } from '@/lib/i18n'
import { jobTypeName } from '@/features/settings/background/job-view'

export type TaskEntry =
  | {
      key: string
      at: number
      agent: KindTask
      semantic?: undefined
      browsing?: undefined
    }
  | {
      key: string
      /** No time: it has none (see `semanticEntry`). */
      at: null
      semantic: SemanticTask
      agent?: undefined
      browsing?: undefined
    }
  | {
      key: string
      at: number
      browsing: BrowsingTask
      agent?: undefined
      semantic?: undefined
    }

/** A row with a time: what `mergeNewestFirst` merges. */
export type TimedEntry = Exclude<TaskEntry, { at: null }>

/** When a conversation backup sorts: started while it works, ended after. */
export function agentEntry(task: KindTask, group: TaskGroup): TimedEntry {
  const at =
    group === 'running' || group === 'waiting'
      ? task.startedAt
      : (task.endedAt ?? task.startedAt)
  return { key: `agent:${task.taskId}`, at, agent: task }
}

/**
 * The index build has no start time, so it takes no part in merging by
 * time: it is listed only while it is going, first in its group (see
 * `useTaskGroup`), and its row shows no time.
 */
export function semanticEntry(task: SemanticTask): TaskEntry {
  return { key: `semantic:${task.taskId}`, at: null, semantic: task }
}

/** A browsing row's `at` is already the time its group sorts by. */
export function browsingEntry(task: BrowsingTask): TimedEntry {
  return { key: task.taskId, at: task.at ?? 0, browsing: task }
}

/** Whether the row is listed under "Needs a look" (so not also under today). */
export function needsLook(entry: TaskEntry) {
  return (entry.agent ?? entry.semantic ?? entry.browsing).attention
}

export interface MergeSource {
  entries: TimedEntry[]
  hasMore: boolean
}

/**
 * The rows that can be shown in order, and which source to read on from.
 * A source with more pages bounds what is shown: anything older than the
 * oldest row it has given could still be beaten by one of its next rows.
 */
export function mergeNewestFirst(sources: readonly MergeSource[]): {
  entries: TimedEntry[]
  /** The source whose next page shows more, or `null` when all are read. */
  next: number | null
} {
  let floor = -Infinity
  let next: number | null = null
  sources.forEach((source, index) => {
    if (!source.hasMore) return
    const oldest = source.entries.reduce(
      (least, entry) => Math.min(least, entry.at),
      Infinity,
    )
    const bound = Number.isFinite(oldest) ? oldest : Infinity
    if (next === null || bound > floor) {
      floor = bound
      next = index
    }
  })
  const entries = sources
    .flatMap((source) => source.entries)
    .filter((entry) => entry.at >= floor)
    .sort((a, b) => b.at - a.at || a.key.localeCompare(b.key))
  return { entries, next }
}

const runNames: Record<string, MessageKey> = {
  backup: 'tasks.browsing.backup',
  import: 'backup.runs.types.import',
  // Work on the whole archive.
  doctor: 'backup.runs.types.doctor',
  rekey: 'backup.runs.types.rekey',
  retention_prune: 'backup.runs.types.retention_prune',
  archive_restore: 'backup.runs.types.archive_restore',
  snapshot_restore: 'backup.runs.types.snapshot_restore',
  rollback: 'backup.runs.types.rollback',
  restore: 'backup.runs.types.restore',
  'same-visit': 'backup.runs.types.same-visit',
}

/** A browsing row's name: what kind of run, or which queue's job. */
export function browsingTaskName(t: Translator, task: BrowsingTask) {
  const source = browsingTaskSource(task)
  if (source === 'run') {
    const key = runNames[task.operation]
    return key ? t(key) : t('tasks.browsing.other', { code: task.operation })
  }
  const name = jobTypeName(t, task.operation)
  return source === 'intelligence'
    ? t('tasks.browsing.job', {
        queue: t('settingsBackground.insights.title'),
        name,
      })
    : name
}

const stateWords: Record<string, MessageKey> = {
  queued: 'settingsBackground.state.queued',
  running: 'settingsBackground.state.running',
  succeeded: 'settingsBackground.state.succeeded',
  failed: 'settingsBackground.state.failed',
  cancelled: 'settingsBackground.state.cancelled',
  paused: 'settingsBackground.state.paused',
  stale: 'settingsBackground.state.stale',
  partial: 'tasks.browsing.state.partial',
  interrupted: 'tasks.browsing.state.interrupted',
}

export function browsingStateText(t: Translator, state: string) {
  const key = stateWords[state]
  return key ? t(key) : t('tasks.browsing.state.other', { code: state })
}

/** A state that has ended, well or not. */
export function browsingEnded(state: string) {
  return !['queued', 'running', 'paused', 'stale'].includes(state)
}
