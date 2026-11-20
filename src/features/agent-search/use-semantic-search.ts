/**
 * One search by meaning in Records, read a page at a time: up to 100
 * passages a page, closest first, the next page when the list asks for it,
 * until the backend says there are no more. Nothing is capped, so the
 * passages live outside React state, in one array each finished page is
 * appended to; the screen is told how many there are, never handed a copy.
 *
 * Responsibilities: starting the search for a request and stopping the one
 * before (new words, other filters, leaving the page); one page at a time,
 * its passages held back until its `finished`, so a page that fails or is
 * asked for again never adds a row twice; stopping the page in flight;
 * asking for a failed, refused or stopped page again from the same cursor;
 * starting again from the first page when the backend says the cursor no
 * longer fits (`stale_cursor`, `invalid-cursor`); telling a typed refusal
 * and an index that isn't ready from other failures, reading the index's
 * status again after either, and asking again by itself once that status
 * changes.
 * Not responsible for: whether to search at all (`semantic-gate.ts`), when
 * the list wants the next page (`semantic-results.tsx`), drawing, or any
 * copy.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  SemanticPassage,
  SemanticSearchFrame,
  SemanticSearchRequest,
} from '@/lib/backend-client/agent-search'
import {
  aiRefusalCode,
  type AiRefusalCode,
} from '@/lib/backend-client/ai-refusals'
import { describeError } from '@/lib/errors'
import {
  openSemanticSearch,
  type AgentReadStream,
} from '@/lib/ipc/agent-read-stream'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryClient } from '@/lib/query'
import { semanticIndexKey } from '@/lib/queries/agent-semantic'

/**
 * The passages so far. `items` grows by a page at a time and is the same
 * array for the whole search; draw only the first `count`.
 */
export interface PassageList {
  items: readonly SemanticPassage[]
  count: number
}

/**
 * - `starting`: the first page is on its way; nothing to show yet.
 * - `more`: pages are in and the backend has more; the list asks for them.
 * - `loading`: a later page is on its way.
 * - `done`: the last page is in.
 * - `stopped`: Stop (or the backend) ended a page before it was in.
 * - `failed`: a page didn't come; `error` says why.
 * - `refused`: a page was refused for a typed reason (`refusal`).
 * - `notReady`: the backend has no index it can search with yet.
 */
export type SemanticPhase =
  | 'starting'
  | 'more'
  | 'loading'
  | 'done'
  | 'stopped'
  | 'failed'
  | 'refused'
  | 'notReady'

export interface SemanticState {
  phase: SemanticPhase
  passages: PassageList
  /** Conversations changed after the index that answered was built. */
  stale: boolean
  /** A stable error code, or a sentence when the call itself failed. */
  error: { code: string; message: string } | null
  refusal: AiRefusalCode | null
  /**
   * The index status (`semantic-gate.ts`'s signature) a refused or
   * not-ready page was sent under; the page is asked for again once it
   * changes.
   */
  under: string | null
}

const NOT_READY = 'agent-semantic-index-not-ready'

const initial: SemanticState = {
  phase: 'starting',
  passages: { items: [], count: 0 },
  stale: false,
  error: null,
  refusal: null,
  under: null,
}

interface Run {
  /** The first page. */
  begin: () => void
  /** The next page, if the list may ask for it now. */
  more: () => void
  stop: () => void
  /** The page that failed, was refused or was stopped, from the same cursor. */
  again: () => void
  end: () => void
}

/**
 * One search from its first page on. `show` merges into the state;
 * `statusNow` is the index status a page is sent under; `restart` throws
 * the run away for a new one.
 */
function startRun(
  request: SemanticSearchRequest,
  show: (change: Partial<SemanticState>) => void,
  statusNow: () => string,
  restart: () => void,
): Run {
  const items: SemanticPassage[] = []
  /** Where the next page starts (`null`: the first page); `null` once the last is in. */
  let next: { cursor: string | null } | null = { cursor: null }
  let stream: AgentReadStream | null = null
  let busy = false
  /** The list may ask for the next page: the last one came in whole. */
  let open = false
  /** Counts pages asked for; a page's frames count only while it is the latest. */
  let page = 0
  let ended = false

  const load = (cursor: string | null) => {
    if (ended || busy) return
    busy = true
    open = false
    const token = ++page
    const current = () => !ended && token === page
    const under = statusNow()
    const arrived: SemanticPassage[] = []
    show({
      phase: cursor === null ? 'starting' : 'loading',
      error: null,
      refusal: null,
      under: null,
    })

    const fail = (code: string, message: string) => {
      busy = false
      stream = null
      // The index, the conversations, their marks or the words changed
      // under the cursor, or it can't be read: only the first page helps.
      if (code === 'stale_cursor' || code === 'invalid-cursor') {
        restart()
        return
      }
      const refusal = aiRefusalCode(code)
      if (refusal || code === NOT_READY)
        // The status may say why now; the gate shows the index's own state.
        void queryClient.invalidateQueries({ queryKey: semanticIndexKey })
      if (refusal) show({ phase: 'refused', refusal, under })
      else if (code === NOT_READY) show({ phase: 'notReady', under })
      else show({ phase: 'failed', error: { code, message } })
    }

    const onFrame = (frame: SemanticSearchFrame) => {
      if (!current()) return
      switch (frame.type) {
        case 'passages':
          for (const passage of frame.payload.items) arrived.push(passage)
          break
        case 'finished': {
          busy = false
          stream = null
          for (const passage of arrived) items.push(passage)
          const { nextCursor } = frame.payload
          next = nextCursor === null ? null : { cursor: nextCursor }
          open = next !== null
          show({
            phase: next ? 'more' : 'done',
            stale: frame.payload.stale,
            passages: { items, count: items.length },
          })
          break
        }
        case 'cancelled':
          busy = false
          stream = null
          show({ phase: 'stopped' })
          break
        case 'error':
          fail(frame.payload.code, frame.payload.message ?? frame.payload.code)
          break
        default:
          break
      }
    }

    openSemanticSearch(
      cursor === null ? request : { ...request, cursor },
      onFrame,
    ).then(
      (started) => {
        if (current() && busy) stream = started
        // Stopped, replaced or already over: closing cancels only a live one.
        else started.close()
      },
      (error: unknown) => {
        if (current())
          fail(commandErrorCode(error) ?? 'failed', describeError(error))
      },
    )
  }

  return {
    begin: () => {
      show({ ...initial, passages: { items, count: 0 } })
      load(null)
    },
    more: () => {
      if (open && next) load(next.cursor)
    },
    stop: () => {
      if (!busy) return
      busy = false
      page++
      stream?.close()
      stream = null
      show({ phase: 'stopped' })
    },
    again: () => {
      if (next) load(next.cursor)
    },
    end: () => {
      ended = true
      page++
      stream?.close()
      stream = null
    },
  }
}

/**
 * `request` is `null` while the index's own state says not to search.
 * `status` is the gate's signature of the index status.
 */
export function useSemanticSearch(
  request: SemanticSearchRequest | null,
  status: string,
) {
  const key = request ? JSON.stringify(request) : null
  const [held, setHeld] = useState<{
    key: string | null
    state: SemanticState
  }>({ key: null, state: initial })
  const run = useRef<Run | null>(null)
  const [restarts, setRestarts] = useState(0)
  const statusRef = useRef(status)
  useEffect(() => {
    statusRef.current = status
  }, [status])

  useEffect(() => {
    if (!key) return
    const wanted = JSON.parse(key) as SemanticSearchRequest
    const show = (change: Partial<SemanticState>) =>
      setHeld((previous) => ({
        key,
        state: {
          ...(previous.key === key ? previous.state : initial),
          ...change,
        },
      }))
    const current = startRun(
      wanted,
      show,
      () => statusRef.current,
      () => setRestarts((n) => n + 1),
    )
    run.current = current
    current.begin()
    return () => {
      current.end()
      if (run.current === current) run.current = null
      // Nothing of this run shows again, even if the same request comes back.
      setHeld({ key: null, state: initial })
    }
  }, [key, restarts])

  // What was drawn for another request (or none) is never shown for this one.
  const state = held.key === key ? held.state : initial

  // Refused, or not ready, under a status that has changed since: ask again.
  const waiting =
    state.phase === 'refused' || state.phase === 'notReady' ? state.under : null
  useEffect(() => {
    if (waiting !== null && waiting !== status) run.current?.again()
  }, [waiting, status])

  /** The next page, when the list nears its end; ignored unless one is due. */
  const more = useCallback(() => run.current?.more(), [])
  /** Stops the page in flight; the passages so far stay. */
  const stop = useCallback(() => run.current?.stop(), [])
  /** The failed, refused or stopped page again, from the same cursor. */
  const retry = useCallback(() => run.current?.again(), [])

  return { state, more, stop, retry }
}
