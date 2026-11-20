/**
 * What the rail's background-task indicator shows, from work the app already
 * follows on every screen.
 *
 * Responsibilities: one list of running tasks and two counts, from
 * - the backup runner (a backup started here, with its progress events);
 * - the Takeout and browser-file imports (`import-runner.tsx`);
 * - the search-index queue's counts and running jobs in the app snapshot,
 *   which is read again when a backup or an import ends, when Settings
 *   starts index work, and when the queue reports a job ended;
 * - the last backup, which needs a look when it failed (the same signal as
 *   Home's backup status), until a backup succeeds;
 * - conversation backups (`get_task_summary`): those running, in the units
 *   they have (no percentage: no total is known until they end), and how
 *   many need a look. The frame keeps that answer current from the
 *   backend's task events (`useAgentBackupSync`); it is read only while
 *   Agent conversations are on or paused and their store opens;
 * - the conversations' semantic index build, which the same answer
 *   carries (passages read so far; its events keep it current through
 *   `useAgentSemanticSync`).
 *
 * It adds no polling. So it leaves out what nothing follows outside the
 * background tasks page (`#/tasks`): the Insights queue's jobs, and a live
 * percentage for an index job (the snapshot's figure would stand still).
 *
 * Not responsible for: drawing (`task-indicator.tsx`), or the page's full
 * lists (`features/tasks/`), which take the window's own work from
 * `useWindowWork`.
 */
import { useMemo } from 'react'
import { useBackupRunner } from '@/app/backup-runner'
import {
  useBrowserImport,
  useTakeoutImport,
} from '@/features/backup/import-runner'
import { useJobViews } from '@/features/settings/background/job-view'
import { taskName, taskProgressText } from '@/features/agent-backup/words'
import type { KindTask } from '@/lib/backend-client/agent-backup'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  useAgentKind,
  useTaskSummary,
  wireNumber,
} from '@/lib/queries/agent-backup'
import { useSnapshot } from '@/lib/queries/app'
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
  queued: number
  /** The last backup when it failed, to open its details. */
  failedRun: number | null
  /** Conversation backups that need a look (the background tasks page lists them). */
  kindAttention: number
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

export function useBackgroundTasks(): BackgroundTasks {
  const { t } = useI18n()
  const format = useFormat()
  const windowWork = useWindowWork()
  const summaryQuery = useTaskSummary()
  // A kind turned off keeps its last answer in the cache; it is not current.
  const summary = useAgentKind().readable ? summaryQuery.data : undefined
  const { aiStatus, recentRuns } = useSnapshot()
  const { fromAiQueue } = useJobViews()

  return useMemo(() => {
    const running: BackgroundTask[] = [...windowWork]

    for (const job of aiStatus.recentJobs) {
      if (job.state !== 'running') continue
      running.push({
        id: `ai-${job.id}`,
        name: fromAiQueue(job).name,
        percent: null,
        preparing: false,
        detail: null,
      })
    }

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

    const last = recentRuns[0]
    return {
      running,
      queued: aiStatus.queuedJobs + (summary?.queuedCount ?? 0),
      failedRun: last?.status === 'failed' ? last.id : null,
      kindAttention: wireNumber(summary?.attentionCount),
    }
  }, [t, format, summary, windowWork, aiStatus, recentRuns, fromAiQueue])
}
