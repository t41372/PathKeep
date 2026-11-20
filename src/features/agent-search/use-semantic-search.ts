/**
 * One running search by meaning in Records: every passage the semantic
 * index finds close to the words, closest first, as the backend streams
 * them. Nothing is capped, so the passages live outside React state, in
 * one array the stream appends to; the screen is told how many there are
 * once per frame, never handed a copy, however many arrive.
 *
 * Responsibilities: starting the search for a request and stopping the one
 * before (new words, other filters, leaving the page); the count while it
 * runs and when it ends; stopping on request (the passages so far stay);
 * starting again when marks changed under it (`stale_cursor`).
 * Not responsible for: whether the index can answer at all
 * (`semantic-gate.ts`), drawing, or any copy.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  SemanticPassage,
  SemanticSearchFrame,
  SemanticSearchRequest,
} from '@/lib/backend-client/agent-search'
import {
  openSemanticSearch,
  type AgentReadStream,
} from '@/lib/ipc/agent-read-stream'
import { describeError } from '@/lib/errors'
import { commandErrorCode } from '@/lib/ipc/command-error'

/**
 * The passages so far. `items` is appended to while the search runs and
 * may already hold more than `count`; draw only the first `count`.
 */
export interface PassageList {
  items: readonly SemanticPassage[]
  count: number
}

export type SemanticPhase =
  'starting' | 'searching' | 'done' | 'stopped' | 'failed'

export interface SemanticState {
  phase: SemanticPhase
  passages: PassageList
  /** Conversations changed after the index that answered was built. */
  stale: boolean
  /** A stable error code, or a sentence when the call itself failed. */
  error: { code: string; message: string } | null
}

const empty: PassageList = { items: [], count: 0 }

const initial: SemanticState = {
  phase: 'starting',
  passages: empty,
  stale: false,
  error: null,
}

export function useSemanticSearch(request: SemanticSearchRequest | null) {
  const key = request ? JSON.stringify(request) : null
  const [state, setState] = useState<SemanticState>(initial)
  const stream = useRef<AgentReadStream | null>(null)
  const [attempt, setAttempt] = useState(0)

  useEffect(() => {
    if (!key) return
    const wanted = JSON.parse(key) as SemanticSearchRequest
    let live = true
    let frame = 0
    let pending: Partial<SemanticState> = {}
    const items: SemanticPassage[] = []
    // eslint-disable-next-line react-hooks/set-state-in-effect -- a new search starts from nothing
    setState({ ...initial, passages: { items, count: 0 } })
    const flush = () => {
      frame = 0
      if (!live) return
      const next = pending
      pending = {}
      setState((previous) => ({
        ...previous,
        ...next,
        passages: { items, count: items.length },
      }))
    }
    const schedule = (change: Partial<SemanticState>) => {
      pending = { ...pending, ...change }
      if (!frame) frame = requestAnimationFrame(flush)
    }
    const onFrame = (event: SemanticSearchFrame) => {
      if (!live) return
      switch (event.type) {
        case 'started':
          schedule({ phase: 'searching' })
          break
        case 'passages':
          for (const passage of event.payload.items) items.push(passage)
          schedule({})
          break
        case 'finished':
          schedule({ phase: 'done', stale: event.payload.stale })
          stream.current = null
          break
        case 'cancelled':
          break
        case 'error':
          stream.current = null
          // Marks or visibility changed while it ran: search again.
          if (event.payload.code === 'stale_cursor') {
            setAttempt((n) => n + 1)
            return
          }
          schedule({
            phase: 'failed',
            error: {
              code: event.payload.code,
              message: event.payload.message ?? event.payload.code,
            },
          })
          break
        default:
          break
      }
    }
    openSemanticSearch(wanted, onFrame).then(
      (started) => {
        if (live) stream.current = started
        else started.close()
      },
      (error: unknown) => {
        if (!live) return
        schedule({
          phase: 'failed',
          error: {
            code: commandErrorCode(error) ?? 'failed',
            message: describeError(error),
          },
        })
      },
    )
    return () => {
      live = false
      if (frame) cancelAnimationFrame(frame)
      stream.current?.close()
      stream.current = null
    }
  }, [key, attempt])

  /** Stops it where it is; the passages so far stay. */
  const stop = useCallback(() => {
    stream.current?.close()
    stream.current = null
    setState((previous) =>
      previous.phase === 'searching' || previous.phase === 'starting'
        ? { ...previous, phase: 'stopped' }
        : previous,
    )
  }, [])

  const retry = useCallback(() => setAttempt((n) => n + 1), [])

  return { state, stop, retry }
}
