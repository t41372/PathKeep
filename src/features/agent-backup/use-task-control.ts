/**
 * Cancel, try again or dismiss a conversation backup, or put away one of
 * its format notices, from wherever it is shown (the rail's task list,
 * Settings › Background work, the save step, a run's details).
 *
 * Responsibilities: one action at a time (the others wait, as the spec
 * asks); sending the task's newest `snapshotToken`, which changes with every
 * phase and every action, and sending once more with a fresh one when the
 * backend says it changed meanwhile; keeping the task the backend answered
 * with in every cached copy, so the next action has its token; saying what
 * happened when an action could not be done; and when a retry needs the
 * folders looked at again, opening that review (`use-open-review.ts`).
 * Not responsible for: which actions a task offers (the backend's `actions`).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  agentBackupClient,
  type ControlResult,
  type FormatNotice,
  type KindTask,
  type TaskAction,
} from '@/lib/backend-client/agent-backup'
import { dataKindsErrorCode } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import {
  cachedTask,
  refetchAfterBackup,
  rememberTask,
} from '@/lib/queries/agent-backup'
import { useOpenReview } from './use-open-review'

/** The task as the backend sees it now: the cached copy after the lists are read again. */
async function freshTask(task: KindTask) {
  await Promise.all([
    queryClient.refetchQueries({ queryKey: queryKeys.kindTasks }),
    queryClient.refetchQueries({
      queryKey: queryKeys.archiveRun(task.runRef),
    }),
  ])
  return cachedTask(task.taskId)
}

/** What `busy` holds while an action is on its way. */
export function actionKey(
  task: Pick<KindTask, 'taskId'>,
  action: TaskAction,
  notice?: FormatNotice,
) {
  return [task.taskId, action, notice?.tool, notice?.version]
    .filter(Boolean)
    .join(':')
}

export function useTaskControl() {
  const { t } = useI18n()
  const openReview = useOpenReview()
  const [busy, setBusy] = useState<string | null>(null)

  const send = async (
    task: KindTask,
    action: TaskAction,
    notice?: FormatNotice,
  ): Promise<ControlResult | null> => {
    if (busy !== null) return null
    setBusy(actionKey(task, action, notice))
    try {
      const latest = cachedTask(task.taskId) ?? task
      const control = (token: string) =>
        agentBackupClient.control(
          task.taskId,
          action,
          token,
          crypto.randomUUID(),
          notice,
        )
      let result: ControlResult
      try {
        result = await control(latest.snapshotToken)
      } catch (error) {
        if (dataKindsErrorCode(error) !== 'policy-changed') throw error
        // It moved to another phase between the read and the press.
        const fresh = await freshTask(task)
        if (!fresh?.actions.includes(action)) {
          toast(t('agentTasks.control.changed'))
          return null
        }
        result = await control(fresh.snapshotToken)
      }
      // The original task answers with its new token; a retry's new task
      // arrives with the lists.
      if (!result.replacementTaskId) rememberTask(result.task)
      if (result.previewRequired) await openReview()
      refetchAfterBackup(task.runRef)
      return result
    } catch (error) {
      switch (dataKindsErrorCode(error)) {
        case 'operation-finishing':
          toast(t('agentTasks.control.finishing'))
          break
        case 'archive-busy':
          toast(t('agentTasks.control.busy'))
          break
        case 'task-action-unavailable':
        case 'policy-changed':
          toast(t('agentTasks.control.changed'))
          break
        case 'lock-required':
          // The lock screen is already on its way.
          break
        default:
          toast.error(t('agentTasks.control.failed'), {
            description: describeError(error, 'control_background_task'),
          })
      }
      refetchAfterBackup(task.runRef)
      return null
    } finally {
      setBusy(null)
    }
  }

  return {
    /** `actionKey(…)` of the action on its way. */
    busy,
    send,
  }
}
