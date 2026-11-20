/**
 * Queries for the run ledger's part of the background tasks page: one
 * group of its work (browsing backups and imports, the Insights and
 * search-index queues' jobs, work on the whole archive) a page at a time;
 * dismissing a row from "Needs a look"; and keeping them and the rail's
 * summary current from the backend's ledger event.
 *
 * Responsibilities: one cached copy per group, read whenever the page asks
 * (with browsing history off the backend answers the whole-archive rows
 * only, so the browsing state is part of the key); reading every list and
 * the summary again after `pathkeep://background-tasks-changed`, gathered
 * into one read per quarter second, since a busy queue sends an event per
 * transaction. Nothing here polls. The keys sit under the archive's, so a
 * finished backup or import refreshes them with everything else; the lock
 * clears them with the rest of the cache.
 * Not responsible for: conversation backups (`agent-backup.ts`), or any copy.
 */
import { useInfiniteQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import type { TaskGroup } from '@/lib/backend-client/agent-backup'
import { backgroundTasksClient } from '@/lib/backend-client/background-tasks'
import { subscribeToBackgroundTasksChanged } from '@/lib/ipc/background-tasks-events'
import { queryClient, queryKeys } from '@/lib/query'
import { useDataKinds } from './data-kinds'

const PAGE = 20
/** How long events are gathered before the lists are read again. */
const GATHER_MS = 250

/** Every ledger group; a backup or import ending refreshes them with the archive. */
export const browsingTasksKey = [
  ...queryKeys.archiveData,
  'browsing-tasks',
] as const

export function useBrowsingTasks(group: TaskGroup) {
  const kinds = useDataKinds()
  const state = kinds.data?.kinds.find((row) => row.kind === 'browsing')?.state
  return useInfiniteQuery({
    queryKey: [...browsingTasksKey, state ?? 'unknown', group],
    queryFn: ({ pageParam }) =>
      backgroundTasksClient.browsing(group, PAGE, pageParam),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor,
    enabled: kinds.data !== undefined,
    // Opening the page reads afresh; while it is open the event does.
    staleTime: 0,
  })
}

/** Reads every ledger group and the rail's summary again: after an action, or an event. */
export function refetchBrowsingTasks() {
  // A read already on its way finishes; restarting it per event would
  // never let a busy queue's list arrive.
  return Promise.all([
    queryClient.invalidateQueries(
      { queryKey: browsingTasksKey },
      { cancelRefetch: false },
    ),
    queryClient.invalidateQueries(
      { queryKey: queryKeys.taskSummary },
      { cancelRefetch: false },
    ),
  ])
}

let gathering: ReturnType<typeof setTimeout> | null = null

function refetchSoon() {
  if (gathering !== null) return
  gathering = setTimeout(() => {
    gathering = null
    void refetchBrowsingTasks()
  }, GATHER_MS)
}

/**
 * Keeps the ledger's lists and the rail's summary current from the
 * backend's event, wherever the work started (this window, the menu bar,
 * the schedule). Mounted once, by the frame, which unmounts on lock.
 */
export function useBackgroundTasksSync() {
  useEffect(() => {
    let stopped = false
    let stop: (() => void) | undefined
    void subscribeToBackgroundTasksChanged(refetchSoon).then((unsubscribe) => {
      if (stopped) {
        unsubscribe()
        return
      }
      stop = unsubscribe
      // What changed before this listener started sent its event to nobody.
      void refetchBrowsingTasks()
    })
    return () => {
      stopped = true
      stop?.()
      if (gathering !== null) clearTimeout(gathering)
      gathering = null
    }
  }, [])
}
