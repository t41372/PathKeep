/**
 * Which first save the enable dialog shows: the one it started in this
 * session (kept until the person has seen how it ended, so closing the
 * dialog and opening it again shows the same run, even once it finished),
 * or else a first save the backend reports running (after a reload, a
 * lock, or a start in another window).
 *
 * Responsibilities: remembering the started save in the query cache, which
 * the lock clears with everything else, and choosing between the two.
 * Not responsible for: starting it (`use-start-saving.ts`) or drawing it.
 */
import { useQuery } from '@tanstack/react-query'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { queryClient, queryKeys } from '@/lib/query'
import { useAgentKind, useTaskSummary } from '@/lib/queries/agent-backup'

export interface ShownSave {
  taskId: string
  runRef: string
  /** The tools ticked when it started; unknown for one found running. */
  tools?: AgentToolId[]
}

export function rememberFirstSave(save: ShownSave) {
  queryClient.setQueryData(queryKeys.firstSave, save)
}

export function forgetFirstSave() {
  queryClient.removeQueries({ queryKey: queryKeys.firstSave })
}

function useRemembered() {
  return useQuery<ShownSave | null>({
    queryKey: queryKeys.firstSave,
    queryFn: () => null,
    // Never fetched: only `rememberFirstSave` writes it.
    enabled: false,
    staleTime: Infinity,
    gcTime: Infinity,
  }).data
}

/**
 * The first save to show, `null` for none; `known` is false until the
 * backend has said whether one runs.
 */
export function useFirstSaveToShow() {
  const remembered = useRemembered()
  const { readable } = useAgentKind()
  const summary = useTaskSummary()
  const running = summary.data?.running.find(
    (task) => task.trigger === 'enable',
  )
  const save: ShownSave | null =
    remembered ??
    (running ? { taskId: running.taskId, runRef: running.runRef } : null)
  return {
    save,
    /** The save was started from this dialog in this session. */
    remembered: remembered != null,
    known: remembered != null || !readable || summary.isFetched,
  }
}
