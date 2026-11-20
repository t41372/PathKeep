/**
 * Rows of one view of one conversation, read a window at a time around
 * what is on screen (`agent-history.md` §4.5, §4.9: opening, scrolling and
 * jumping never read the whole conversation). Windows are fixed slices of
 * ranks (`WINDOW` rows each), so the same slice is one cache entry however
 * it was reached, and a slice already read stays while others load.
 *
 * Responsibilities: which windows the visible ranks need, reading them
 * (cancelling a read when it scrolls out of need), each rank's row or its
 * state (loading, failed), the view's length, and asking for a fresh
 * header when the publication moved on (`stale_cursor`). Not responsible
 * for: drawing.
 */
import { useQueries } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo } from 'react'
import {
  agentChatsClient,
  type ChatView,
  type ChatWindow,
} from '@/lib/backend-client/agent-chats'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryClient, queryKeys } from '@/lib/query'

export const WINDOW = 100

export type RankState<Row> =
  | { state: 'row'; row: Row }
  | { state: 'loading' }
  | { state: 'failed'; error: Error; retry: () => void }

interface Options {
  id: string
  publication: string
  view: ChatView
  /** A Pi leaf; `null` for the line the tool used last. */
  leaf: number | null
  /** The view's length before a window says (the header's counts). */
  knownTotal: number
  /** The ranks on screen, overscan included; `null` before the first draw. */
  range: { first: number; last: number } | null
  /** Called when the publication moved on: read the header again. */
  onStale: () => void
}

function lineKey(leaf: number | null) {
  return leaf === null ? 'current' : `leaf:${leaf}`
}

export function useWindows<Row>({
  id,
  publication,
  view,
  leaf,
  knownTotal,
  range,
  onStale,
}: Options) {
  const line = lineKey(leaf)
  const keyOf = useCallback(
    (index: number) =>
      queryKeys.agentChatWindow(id, publication, view, line, index),
    [id, publication, view, line],
  )
  const indexes = useMemo(() => {
    const first = range ? Math.floor(range.first / WINDOW) : 0
    const last = range ? Math.floor(range.last / WINDOW) : 0
    const list: number[] = []
    for (let index = first; index <= last; index += 1) list.push(index)
    return list
  }, [range])

  const results = useQueries({
    queries: indexes.map((index) => ({
      queryKey: keyOf(index),
      queryFn: async ({ signal }: { signal: AbortSignal }) => {
        const requestId = crypto.randomUUID()
        const cancel = () =>
          void agentChatsClient.cancel(requestId).catch(() => {})
        signal.addEventListener('abort', cancel, { once: true })
        try {
          return await agentChatsClient.window<Row>({
            id,
            publication,
            view,
            ...(leaf === null ? {} : { leaf }),
            anchor: { kind: 'rank', rank: index * WINDOW },
            limit: WINDOW,
            requestId,
          })
        } finally {
          signal.removeEventListener('abort', cancel)
        }
      },
      staleTime: Infinity,
      retry: false,
    })),
  })

  const stale = results.some(
    (result) => commandErrorCode(result.error) === 'stale_cursor',
  )
  useEffect(() => {
    if (stale) onStale()
  }, [stale, onStale])

  // The newest answer's length wins: a leaf's line is only known once read.
  let total = knownTotal
  for (const result of results) if (result.data) total = result.data.total

  const rankState = useCallback(
    (rank: number): RankState<Row> => {
      const index = Math.floor(rank / WINDOW)
      const key = keyOf(index)
      const data = queryClient.getQueryData<ChatWindow<Row>>(key)
      if (data) {
        const row = data.rows[rank - data.first]
        if (row !== undefined) return { state: 'row', row }
      }
      const error = queryClient.getQueryState(key)?.error
      if (error)
        return {
          state: 'failed',
          error: error instanceof Error ? error : new Error(String(error)),
          retry: () => void queryClient.refetchQueries({ queryKey: key }),
        }
      return { state: 'loading' }
    },
    // `results` changes whenever a window arrives, so rows are read again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [keyOf, results],
  )

  return { total, rankState }
}
