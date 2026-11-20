/**
 * Pause, continue, cancel, try again or dismiss on a ledger row of the
 * background tasks page: each row names the command its state allows. A
 * queue command is called the way the queue's own list does it (the same
 * notices, the same queues read again); dismissing takes a failed or
 * partial row out of "Needs a look" (its history stays). Then the page's
 * ledger groups and the rail's summary are read again.
 *
 * Responsibilities: one command at a time, and which one is on its way.
 * Not responsible for: which commands a row offers (the backend's
 * `commands`), or conversation backups (`use-task-control.ts`).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  backgroundTasksClient,
  type BrowsingTask,
  type BrowsingTaskCommand,
} from '@/lib/backend-client/background-tasks'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { refetchBrowsingTasks } from '@/lib/queries/background-tasks'
import {
  useAiJobActions,
  useRuntimeJobActions,
} from '@/features/settings/background/use-background-work'

export function browsingActionKey(
  task: Pick<BrowsingTask, 'taskId'>,
  command: BrowsingTaskCommand,
) {
  return `${task.taskId}:${command}`
}

export function useBrowsingControl() {
  const { t } = useI18n()
  const runtime = useRuntimeJobActions()
  const ai = useAiJobActions()
  const [busy, setBusy] = useState<string | null>(null)

  const send = async (task: BrowsingTask, command: BrowsingTaskCommand) => {
    if (busy !== null) return
    setBusy(browsingActionKey(task, command))
    const { jobId } = task
    try {
      switch (command) {
        case 'retry_intelligence_job':
          await runtime.retry.mutateAsync(jobId)
          break
        case 'cancel_intelligence_job':
          await runtime.cancel.mutateAsync({
            id: jobId,
            state: task.state === 'running' ? 'running' : 'queued',
          })
          break
        case 'pause_ai_job':
          await ai.pause.mutateAsync(jobId)
          break
        case 'cancel_ai_job':
          await ai.cancel.mutateAsync(jobId)
          break
        case 'replay_ai_job':
          await ai.replay.mutateAsync(jobId)
          break
        case 'dismiss':
          await backgroundTasksClient.dismiss(task.taskId).catch((error) => {
            toast.error(t('agentTasks.control.failed'), {
              description: describeError(error),
            })
          })
          break
      }
    } catch {
      // A queue command's own notice already said what went wrong.
    } finally {
      setBusy(null)
      void refetchBrowsingTasks()
    }
  }

  return { busy, send }
}
