/**
 * Cancel, try again or dismiss a conversation backup, from wherever it is
 * shown (the rail's task list, Settings › Background work, the save step).
 *
 * Responsibilities: one action at a time (the others wait, as the spec
 * asks); sending the task's newest `snapshotToken`, which changes with every
 * phase, and sending once more with a fresh one when the backend says it
 * changed meanwhile; saying what happened when an action could not be done,
 * including a retry that needs the folders looked at again; refreshing what
 * the action changed.
 * Not responsible for: which actions a task offers (the backend's `actions`).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  agentBackupClient,
  type ControlResult,
  type KindTask,
  type TaskAction,
} from '@/lib/backend-client/agent-backup'
import { dataKindsErrorCode } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import { cachedTask, refetchAfterBackup } from '@/lib/queries/agent-backup'
import { spaceBackupPath } from '@/lib/spaces'

/** The task as the backend sees it now: the cached copy after the lists are read again. */
async function freshTask(taskId: string) {
  await queryClient.refetchQueries({ queryKey: queryKeys.kindTasks })
  return cachedTask(taskId)
}

export function useTaskControl() {
  const { t } = useI18n()
  const [busy, setBusy] = useState<string | null>(null)

  const send = async (
    task: KindTask,
    action: TaskAction,
  ): Promise<ControlResult | null> => {
    if (busy !== null) return null
    setBusy(`${task.taskId}:${action}`)
    try {
      const latest = cachedTask(task.taskId) ?? task
      let result: ControlResult
      try {
        result = await agentBackupClient.control(
          task.taskId,
          action,
          latest.snapshotToken,
          crypto.randomUUID(),
        )
      } catch (error) {
        if (dataKindsErrorCode(error) !== 'policy-changed') throw error
        // It moved to another phase between the read and the press.
        const fresh = await freshTask(task.taskId)
        if (!fresh?.actions.includes(action)) {
          toast(t('agentTasks.control.changed'))
          return null
        }
        result = await agentBackupClient.control(
          task.taskId,
          action,
          fresh.snapshotToken,
          crypto.randomUUID(),
        )
      }
      if (result.previewRequired) {
        toast(t('agentTasks.control.previewRequired'), {
          duration: 10_000,
          action: {
            label: t('agentTasks.control.openBackupPage'),
            onClick: () => {
              window.location.hash = spaceBackupPath('agent')
            },
          },
        })
      }
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
    /** `<taskId>:<action>` while it is on its way. */
    busy,
    send,
  }
}
