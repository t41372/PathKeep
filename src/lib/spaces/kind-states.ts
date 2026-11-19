/**
 * The backend's answer to "which kinds of data are on", as the shell reads
 * it outside React (the switcher's order, ⌘1…⌘9, the last space) and inside.
 *
 * Responsibilities: reading `list_data_kinds`'s cached answer and telling
 * subscribers when it changes. The answer lives in the query cache
 * (`queryKeys.dataKinds`), so the lock and a restart, which clear that
 * cache, also clear it here: nothing from before a lock is shown after it.
 * Not responsible for: fetching it or refetching after the backend's event
 * (`src/lib/queries/data-kinds.ts`, mounted by the frame).
 */
import { hashKey } from '@tanstack/react-query'
import { useSyncExternalStore } from 'react'
import type {
  DataKindId,
  DataKindList,
  KindState,
} from '@/lib/backend-client/data-kinds'
import { queryClient, queryKeys } from '@/lib/query'

/** `undefined` until the backend has answered, and after a lock cleared the answer. */
export type KindListAnswer = DataKindList | undefined

const KEY_HASH = hashKey(queryKeys.dataKinds)

export function readKindList(): KindListAnswer {
  return queryClient.getQueryData<DataKindList>(queryKeys.dataKinds)
}

function subscribe(listener: () => void) {
  return queryClient.getQueryCache().subscribe((event) => {
    if (event.query.queryHash === KEY_HASH) listener()
  })
}

/** The answer, re-rendering when it changes; reading it does not fetch it. */
export function useKindList(): KindListAnswer {
  return useSyncExternalStore(subscribe, readKindList)
}

/** A kind's state in `list`, or `undefined` when the backend has not said. */
export function kindStateOf(
  list: KindListAnswer,
  kind: DataKindId,
): KindState | undefined {
  return list?.kinds.find((row) => row.kind === kind)?.state
}
