/**
 * What the rail's background-task indicator shows, for every kind at once.
 *
 * Responsibilities: one list of running tasks and two counts, from
 * - the backend's summary (`get_task_summary`): running work of every kind
 *   (browsing backups and imports, the Insights and search-index queues,
 *   work on the whole archive, conversation backups, the conversations'
 *   semantic index build), and its own totals of what waits and what needs
 *   a look. The frame keeps it current from the backend's events: the
 *   ledger's (`useBackgroundTasksSync`), conversation backups'
 *   (`useAgentBackupSync`) and the semantic build's
 *   (`useAgentSemanticSync`);
 * - this window's own browsing backup and imports, which report progress
 *   as they go: their figures go on the ledger's row of the same run type
 *   (`useLiveWork`), and one started a moment ago is listed before the
 *   ledger has been read again.
 *
 * It adds no polling and no reads of its own. Conversation backups and the
 * semantic build show no percentage: no total is known until they end.
 *
 * Not responsible for: drawing (`task-indicator.tsx`), or the page's full
 * lists (`features/tasks/`), which take the window's own work from
 * `useLiveWork` too.
 */
import { useMemo } from 'react'
import { useBackupRunner } from '@/app/backup-runner'
import {
  useBrowserImport,
  useTakeoutImport,
} from '@/features/backup/import-runner'
import { taskName, taskProgressText } from '@/features/agent-backup/words'
import { browsingTaskName } from '@/features/tasks/task-entry'
import type { KindTask } from '@/lib/backend-client/agent-backup'
import {
  browsingTaskSource,
  type BrowsingTask,
} from '@/lib/backend-client/background-tasks'
import {
  useFormat,
  useI18n,
  type Formatters,
  type Translator,
} from '@/lib/i18n'
import { useTaskSummary, wireNumber } from '@/lib/queries/agent-backup'
import type { ImportProgressEvent } from '@/lib/types'

export interface BackgroundTask {
  id: string
  name: string
  /** 0–100, or null while the total is not known. */
  percent: number | null
  /** Not started reporting yet: "Preparing" instead of a percentage. */
  preparing: boolean
  detail: string | null
  /** A conversation backup: the list offers its own Cancel. */
  kindTask?: KindTask
}

export interface BackgroundTasks {
  running: BackgroundTask[]
  /** Waiting work of every kind, paused work included. */
  queued: number
  /**
   * What the background tasks page lists under "Needs a look", every kind
   * counted: failed or partial work nobody dismissed, conversation backups
   * that did not save everything, format notices.
   */
  attention: number
}

function clampPercent(value: number | null | undefined) {
  return value == null || !Number.isFinite(value)
    ? null
    : Math.min(100, Math.max(0, value))
}

/**
 * Browsing work this window started and follows live: a backup with its
 * progress events, and the two imports. The background tasks page puts
 * these figures on the matching rows of the run ledger.
 */
export function useWindowWork(): BackgroundTask[] {
  const { t } = useI18n()
  const backup = useBackupRunner()
  const takeout = useTakeoutImport().state
  const browserFile = useBrowserImport().state

  return useMemo(() => {
    const running: BackgroundTask[] = []

    if (backup.running) {
      const progress = backup.progress
      const total = progress?.totalProfiles ?? 0
      running.push({
        id: 'backup',
        name: t('spaces.tasks.backup'),
        // The same figure Backup's progress card shows.
        percent: clampPercent(
          progress?.progressPercent ??
            (progress && total > 0
              ? (progress.completedProfiles / total) * 100
              : null),
        ),
        preparing: !progress,
        detail:
          progress && total > 0
            ? [
                t('shell.backup.runningDetail', {
                  done: Math.min(progress.completedProfiles + 1, total),
                  total,
                }),
                progress.sourceLabel,
              ]
                .filter(Boolean)
                .join(' · ')
            : null,
      })
    }

    const importTask = (
      id: string,
      name: string,
      progress: ImportProgressEvent | null,
    ): BackgroundTask => ({
      id,
      name,
      percent: clampPercent(
        progress?.progressPercent ??
          (progress && progress.total > 0
            ? (progress.current / progress.total) * 100
            : null),
      ),
      preparing: !progress,
      detail:
        progress?.processedRecords != null && progress.totalRecords
          ? t('spaces.tasks.importProgress', {
              done: progress.processedRecords,
              total: progress.totalRecords,
            })
          : null,
    })
    if (takeout.step === 'importing')
      running.push(
        importTask('takeout', t('spaces.tasks.takeout'), takeout.progress),
      )
    if (browserFile.step === 'importing')
      running.push(
        importTask(
          'browser-file',
          t('spaces.tasks.browserFile'),
          browserFile.progress,
        ),
      )
    return running
  }, [t, backup.running, backup.progress, takeout, browserFile])
}

/**
 * This window's own browsing work, by the run type the ledger gives it
 * (`backup`, `import`): its live figures go on the ledger's row of the
 * same type, so it is listed once, with what this window knows.
 */
export type LiveWork = Partial<Record<string, BackgroundTask>>

export function useLiveWork(): LiveWork {
  const windowWork = useWindowWork()
  return useMemo(
    () => ({
      backup: windowWork.find((work) => work.id === 'backup'),
      import: windowWork.find((work) => work.id !== 'backup'),
    }),
    [windowWork],
  )
}

/** Running work on the run ledger, as the rail lists it. */
function ledgerTask(
  t: Translator,
  format: Formatters,
  task: BrowsingTask,
  live: BackgroundTask | undefined,
): BackgroundTask {
  const name = browsingTaskName(t, task)
  if (live) return { ...live, id: `ledger-${task.taskId}`, name: live.name }
  const progress = task.progress
  return {
    id: `ledger-${task.taskId}`,
    name,
    percent:
      progress?.total && progress.total > 0
        ? clampPercent((progress.done / progress.total) * 100)
        : null,
    preparing: false,
    detail: progress
      ? progress.total
        ? t('tasks.row.progress', {
            done: format.number(progress.done),
            total: format.number(progress.total),
          })
        : t('tasks.row.done', { count: progress.done })
      : null,
  }
}

export function useBackgroundTasks(): BackgroundTasks {
  const { t } = useI18n()
  const format = useFormat()
  const windowWork = useWindowWork()
  const live = useLiveWork()
  const summary = useTaskSummary().data

  return useMemo(() => {
    const running: BackgroundTask[] = []
    const shown = new Set<string>()

    // The ledger's running rows, with this window's live figures on its own.
    for (const task of summary?.ledger ?? []) {
      const mine =
        task.kind === 'browsing' && browsingTaskSource(task) === 'run'
          ? live[task.operation]
          : undefined
      if (mine) shown.add(mine.id)
      running.push(ledgerTask(t, format, task, mine))
    }
    // Started here a moment ago: the ledger has not been read again yet.
    for (const work of windowWork) if (!shown.has(work.id)) running.push(work)

    for (const task of summary?.running ?? []) {
      running.push({
        id: `kind-${task.taskId}`,
        name: taskName(t, task),
        percent: null,
        preparing: false,
        detail: taskProgressText(t, format.bytes, task),
        kindTask: task,
      })
    }

    // A paused build waits; the backend counts it in `queuedCount`.
    const semantic = summary?.semantic
    if (semantic && semantic.state !== 'paused')
      running.push({
        id: `semantic-${semantic.taskId}`,
        name: t('agentTasks.names.semanticIndex'),
        percent: null,
        preparing: false,
        detail:
          semantic.state === 'cancelling'
            ? t('agentTasks.progress.stopping')
            : t('agentTasks.progress.passages', {
                count: Number(semantic.progress.done),
              }),
      })

    return {
      running,
      queued: summary?.queuedCount ?? 0,
      attention: wireNumber(summary?.attentionCount),
    }
  }, [t, format, summary, windowWork, live])
}
