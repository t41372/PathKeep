/**
 * Holds one streamed search for the History list.
 *
 * Responsibilities:
 * - Start a search session when the request changes, and close the old one.
 * - Fold steps into pages: a step replaces the earlier steps of its page.
 * - Ask for the next page with the settled page's cursor.
 * - Keep the previous rows (dimmed) until the new session's first step lands.
 * - Restart when an archive write or a note or tag edit invalidates searches,
 *   and once when an index that was still catching up finishes.
 *
 * Not responsible for: wording or layout (search-status.tsx), the timeline or
 * semantic search (queries.ts).
 *
 * Failure modes this guards against: a slow earlier session's events landing
 * in a newer search (every event is checked against the live session);
 * re-rendering on every step on a fast stream (steps are applied once per
 * animation frame); treating an empty step from a partly built index as "no
 * results" (coverage is reported separately).
 */
import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  searchClient,
  type IndexerState,
  type SearchEvent,
  type SearchRequest,
  type SearchStep,
} from '@/lib/backend-client/search'
import { queryKeys } from '@/lib/query'
import type { SearchStream } from '@/lib/ipc/search-stream'

export interface SearchStreamState {
  /** The session these pages came from: request, revision and restart count. */
  session: string | null
  pages: SearchStep[]
  indexer: IndexerState | null
  error: string | null
  /** The next page was asked for and hasn't started arriving. */
  loadingNext: boolean
}

/** How often the index's progress is checked while results are incomplete. */
const INDEXER_POLL_MS = 1000

const EMPTY: SearchStreamState = {
  session: null,
  pages: [],
  indexer: null,
  error: null,
  loadingNext: false,
}

/** The revision bumps whenever searches are invalidated (see `invalidateSearches`). */
export const searchRevisionKey = [...queryKeys.archiveData, 'search-revision']

function apply(
  state: SearchStreamState,
  event: SearchEvent,
): SearchStreamState {
  switch (event.type) {
    case 'indexer':
      return { ...state, indexer: event.data }
    case 'step': {
      const step = event.data
      const pages = state.pages.slice(0, step.page)
      pages[step.page - 1] = step
      return {
        ...state,
        pages,
        indexer: step.indexer,
        loadingNext: state.loadingNext && step.page < pages.length,
      }
    }
    case 'error':
      return { ...state, error: event.data, loadingNext: false }
    case 'cancelled':
      return state
  }
}

export function useSearchStream(request: SearchRequest | null) {
  const requestKey = request ? JSON.stringify(request) : null
  const revision = useQuery({
    queryKey: searchRevisionKey,
    queryFn: () => Date.now(),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  }).data
  const [state, setState] = useState<SearchStreamState>(EMPTY)
  const [restarts, setRestarts] = useState(0)
  const stream = useRef<SearchStream | null>(null)
  const session =
    requestKey === null ? null : `${requestKey}#${revision ?? 0}#${restarts}`

  useEffect(() => {
    if (!request || session === null) {
      setState(EMPTY)
      return
    }
    let live = true
    let queued: SearchEvent[] = []
    let frame = 0
    // Steps can arrive much faster than the screen refreshes; apply them once a frame.
    const flush = () => {
      frame = 0
      const events = queued
      queued = []
      if (!live || events.length === 0) return
      setState((previous) => {
        // Rows of the previous session stay (as placeholder) until this one's first step.
        const base =
          previous.session === session
            ? previous
            : { ...EMPTY, session, indexer: previous.indexer }
        return events.reduce(apply, base)
      })
    }
    const onEvent = (event: SearchEvent) => {
      if (!live) return
      queued.push(event)
      if (frame === 0) frame = requestAnimationFrame(flush)
    }
    setState((previous) => ({ ...previous, error: null, loadingNext: false }))
    void searchClient
      .start(request, onEvent)
      .then((started) => {
        if (!live) {
          started.close()
          return
        }
        stream.current = started
        setState((previous) =>
          previous.session === session
            ? previous
            : { ...previous, indexer: started.indexer },
        )
      })
      .catch((error: unknown) => {
        if (!live) return
        setState({
          ...EMPTY,
          session,
          error: error instanceof Error ? error.message : String(error),
        })
      })
    return () => {
      live = false
      if (frame !== 0) cancelAnimationFrame(frame)
      stream.current?.close()
      stream.current = null
    }
    // `request` is captured through `session`, which also changes on every restart.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session])

  const current = state.session === session ? state : null
  const lastPage = current?.pages.at(-1)

  // While the index hasn't caught up, follow its progress: the session's own
  // events stop once its pages are settled.
  const behind = lastPage !== undefined && !lastPage.coverageComplete
  useEffect(() => {
    if (!behind) return
    const timer = setInterval(() => {
      void searchClient
        .indexerState()
        .then((indexer) =>
          setState((previous) =>
            previous.session === session ? { ...previous, indexer } : previous,
          ),
        )
        .catch(() => undefined)
    }, INDEXER_POLL_MS)
    return () => clearInterval(timer)
  }, [behind, session])

  // A session reads the index as it was when it started. When an index that was
  // still catching up finishes, search once more so the missing pages appear.
  // Once per request, so a search can never restart itself in a loop.
  const caughtUp =
    behind &&
    current?.indexer?.phase === 'ready' &&
    current.indexer.appliedSeq >= current.indexer.targetSeq
  const restartedFor = useRef<string | null>(null)
  useEffect(() => {
    if (!caughtUp || restartedFor.current === requestKey) return
    restartedFor.current = requestKey
    setRestarts((n) => n + 1)
  }, [caughtUp, requestKey])

  const fetchNextPage = useCallback(() => {
    const active = stream.current
    const cursor = lastPage?.cursor
    if (!active || !cursor || current?.loadingNext) return
    setState((previous) => ({ ...previous, loadingNext: true }))
    // A cursor is single-use; if asking fails, the next search starts fresh.
    void searchClient
      .nextPage(active.sessionId, cursor)
      .catch((error: unknown) =>
        setState((previous) => ({
          ...previous,
          loadingNext: false,
          error: error instanceof Error ? error.message : String(error),
        })),
      )
  }, [lastPage?.cursor, current?.loadingNext])

  return useMemo(
    () => ({
      /** Pages of the current request, or null until its first step. */
      current,
      /** Rows still shown from an earlier session while this one starts. */
      placeholder: current === null && state.session !== null ? state : null,
      hasNextPage: Boolean(lastPage?.cursor),
      fetchNextPage,
      refetch: () => setRestarts((n) => n + 1),
    }),
    [current, state, lastPage?.cursor, fetchNextPage],
  )
}
