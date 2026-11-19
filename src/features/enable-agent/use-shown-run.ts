/**
 * The first save the dialog shows, as it runs and once it ended: its task
 * (live progress, patched by the backend's events) and its run (the result,
 * and each tool's outcome).
 *
 * Responsibilities: reading both, noticing the end (an event, or the task
 * leaving the running list), and reading again now and then while it runs,
 * so a missed event cannot leave the dialog waiting forever.
 * Not responsible for: drawing (`save-step.tsx`).
 */
import { useEffect } from 'react'
import { queryClient, queryKeys } from '@/lib/query'
import {
  joinRunPages,
  useArchiveRun,
  useTaskSummary,
} from '@/lib/queries/agent-backup'
import { inToolOrder } from '@/features/data-kinds/tools'
import type { ShownSave } from './first-save'

/** How often to read again while it runs, in case an event went missing. */
const CHECK_MS = 10_000

export function useShownRun(save: ShownSave) {
  const summary = useTaskSummary()
  const task = summary.data?.running.find((item) => item.taskId === save.taskId)
  const run = useArchiveRun(save.runRef)
  const detail = joinRunPages(run.data)
  const result = detail?.result
  const finished = result !== undefined && result !== 'running'

  // Gone from the running list while its run still says running: it ended
  // between the two reads.
  const ended = summary.data !== undefined && !task && result === 'running'
  const { refetch } = run
  useEffect(() => {
    if (ended) void refetch()
  }, [ended, refetch])

  useEffect(() => {
    if (finished) return
    const timer = setInterval(() => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.archiveRun(save.runRef),
      })
      void queryClient.invalidateQueries({ queryKey: queryKeys.taskSummary })
    }, CHECK_MS)
    return () => clearInterval(timer)
  }, [finished, save.runRef])

  const tools =
    save.tools ??
    inToolOrder(
      detail?.perTool?.map((item) => item.tool) ??
        task?.perTool?.map((item) => item.tool) ??
        [],
    )

  return {
    task,
    detail,
    finished,
    tools,
    loading: run.isPending,
    error: run.error,
    retryRead: () => void refetch(),
  }
}
