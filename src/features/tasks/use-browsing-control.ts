/**
 * Pause, continue, cancel or try again on a browsing row of the background
 * tasks page: each row names the queue command its state allows, and this
 * calls it the way the queue's own list does (the same notices, the same
 * queues read again), then reads the page's browsing groups again.
 *
 * Responsibilities: one command at a time, and which one is on its way.
 * Not responsible for: which commands a row offers (the backend's
 * `commands`), or conversation backups (`use-task-control.ts`).
 */
import { useState } from 'react'
import type {
  BrowsingTask,
  BrowsingTaskCommand,
} from '@/lib/backend-client/background-tasks'
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
      }
    } catch {
      // The command's own notice already said what went wrong.
    } finally {
      setBusy(null)
      void refetchBrowsingTasks()
    }
  }

  return { busy, send }
}
