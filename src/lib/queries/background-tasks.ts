/**
 * Queries for browsing history's part of the background tasks page: one
 * group of its work (backups and imports from the run ledger, the Insights
 * and search-index queues' jobs) a page at a time.
 *
 * Responsibilities: one cached copy per group, read only while browsing
 * history is on or paused (the backend answers nothing while it is off);
 * reading every loaded page again every two seconds while the caller says
 * work is under way, or while something is in the running group, since
 * nothing announces a change to these rows. The
 * keys sit under the archive's, so a finished backup or import refreshes
 * them with everything else. The lock clears them with the rest of the
 * cache.
 * Not responsible for: conversation backups (`agent-backup.ts`), or any copy.
 */
import { useInfiniteQuery } from '@tanstack/react-query'
import type { TaskGroup } from '@/lib/backend-client/agent-backup'
import { backgroundTasksClient } from '@/lib/backend-client/background-tasks'
import { queryClient, queryKeys } from '@/lib/query'
import { useDataKinds } from './data-kinds'

const PAGE = 20
/** The pace the queue rows keep while their queues work. */
const POLL_MS = 2_000

/** Every browsing group; a backup or import ending refreshes them with the archive. */
export const browsingTasksKey = [
  ...queryKeys.archiveData,
  'browsing-tasks',
] as const

export function useBrowsingTasks(group: TaskGroup, poll: boolean) {
  const kinds = useDataKinds()
  const state = kinds.data?.kinds.find((row) => row.kind === 'browsing')?.state
  return useInfiniteQuery({
    queryKey: [...browsingTasksKey, group],
    queryFn: ({ pageParam }) =>
      backgroundTasksClient.browsing(group, PAGE, pageParam),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor,
    // Browsing history is on until the backend says otherwise.
    enabled: state !== 'off',
    staleTime: 0,
    // Something running keeps its own group current, wherever it was
    // started (the menu bar, the schedule, another window).
    refetchInterval: (query) =>
      poll ||
      (group === 'running' &&
        (query.state.data?.pages.some((page) => page.items.length > 0) ??
          false))
        ? POLL_MS
        : false,
  })
}

/** Reads every browsing group again: after an action, or when work stops. */
export function refetchBrowsingTasks() {
  return queryClient.invalidateQueries({ queryKey: browsingTasksKey })
}
