/**
 * Queries for the kinds of data PathKeep keeps: their states, the Agent
 * tools' switches, and which source folders exist on this computer.
 *
 * Responsibilities: one cached copy of each answer, and refetching the
 * states and the switches after the backend's change event, whichever
 * window or process made the change. Not responsible for: the shell's
 * synchronous read of the states (`src/lib/spaces/kind-states.ts`) or any
 * screen's wording.
 */
import { useQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import {
  dataKindsClient,
  type DataKindId,
} from '@/lib/backend-client/data-kinds'
import { subscribeToDataKindsChanged } from '@/lib/ipc/data-kinds-events'
import { queryClient, queryKeys } from '@/lib/query'

/** Each kind's state and how much of it the archive holds. */
export function useDataKinds() {
  return useQuery({
    queryKey: queryKeys.dataKinds,
    queryFn: dataKindsClient.list,
    // The change event and a finished backup refresh it.
    staleTime: Infinity,
  })
}

/** One kind's state (on, paused, off), or undefined until the states are known. */
export function useKindState(kind: DataKindId) {
  return useDataKinds().data?.kinds.find((row) => row.kind === kind)?.state
}

function refetchPolicy() {
  void queryClient.invalidateQueries({ queryKey: queryKeys.dataKinds })
  void queryClient.invalidateQueries({ queryKey: queryKeys.agentSettings })
}

/**
 * Keeps the states current for the shell: asks once, then again after
 * every change event. Mounted once, by the frame, which unmounts on lock;
 * the lock clears the answer and the next unlock asks again.
 */
export function useDataKindsSync() {
  useDataKinds()
  useEffect(() => {
    let stopped = false
    let stop: (() => void) | undefined
    void subscribeToDataKindsChanged(refetchPolicy).then((unsubscribe) => {
      if (stopped) {
        unsubscribe()
        return
      }
      stop = unsubscribe
      // A change made between the first answer and this listener's start
      // sent its event to nobody.
      refetchPolicy()
    })
    return () => {
      stopped = true
      stop?.()
    }
  }, [])
}

/** The Agent tools' switches and reading `.git`, with the token a change must carry. */
/**
 * The kinds again with each one's size in the archive, for Manage data
 * types. Counting reads the archive files, so it is asked for only once the
 * cards are drawn, and read again whenever the kinds are (same key prefix).
 */
export function useDataKindSizes(enabled = true) {
  return useQuery({
    queryKey: queryKeys.dataKindSizes,
    queryFn: dataKindsClient.listWithSizes,
    enabled,
    staleTime: Infinity,
    retry: false,
  })
}

export function useAgentSettings(enabled = true) {
  return useQuery({
    queryKey: queryKeys.agentSettings,
    queryFn: dataKindsClient.agentSettings,
    staleTime: Infinity,
    enabled,
  })
}

/**
 * Which source folders of `kinds` exist on this computer. Only checks that
 * a folder is there; nothing inside it is opened or listed. Asked again
 * each time a screen that shows it opens, since tools come and go.
 */
export function useDetectedSources(kinds: DataKindId[], enabled = true) {
  return useQuery({
    queryKey: queryKeys.dataSources(kinds),
    queryFn: () => dataKindsClient.detectSources(kinds),
    staleTime: 0,
    enabled,
  })
}
