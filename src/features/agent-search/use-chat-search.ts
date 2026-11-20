/**
 * One running Records search (`agent-history.md` §3.4): it streams every
 * matching conversation, never capped, and this hook keeps them in arrival
 * order while the screen stays responsive however many arrive.
 *
 * Responsibilities: starting the search for a request and stopping the one
 * before (a new query, other filters, leaving the page); keeping the cards
 * outside React state and showing them once per frame; the numbers while
 * it runs and when it ends; stopping on request (the cards so far stay);
 * starting again when the archive changed under it (`stale_cursor`).
 * Not responsible for: drawing, or any copy.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  SearchCard,
  SearchFrame,
  SearchRequest,
} from '@/lib/backend-client/agent-search'
import {
  openConversationSearch,
  type AgentReadStream,
} from '@/lib/ipc/agent-read-stream'
import { describeError } from '@/lib/errors'
import { commandErrorCode } from '@/lib/ipc/command-error'

/** The cards in arrival order, and each by its id. */
export interface SearchCards {
  order: string[]
  byId: Map<string, SearchCard>
}

export type SearchPhase =
  'starting' | 'searching' | 'done' | 'stopped' | 'failed'

export interface SearchState {
  phase: SearchPhase
  /** Conversations the search looks through (after filters). */
  total: number | null
  /** Conversations the index has not caught up with (a backup is running). */
  indexLag: number
  scanned: number
  conversations: number
  hits: number
  filtered: boolean
  /** A stable error code, or a sentence when the call itself failed. */
  error: { code: string; message: string } | null
  /** The matching conversations so far, in arrival order. */
  cards: SearchCards
}

const initial: SearchState = {
  phase: 'starting',
  total: null,
  indexLag: 0,
  scanned: 0,
  conversations: 0,
  hits: 0,
  filtered: false,
  error: null,
  cards: { order: [], byId: new Map() },
}

export function useChatSearch(request: SearchRequest | null) {
  const key = request ? JSON.stringify(request) : null
  const [state, setState] = useState<SearchState>(initial)
  const stream = useRef<AgentReadStream | null>(null)
  const [attempt, setAttempt] = useState(0)

  useEffect(() => {
    if (!key) return
    const wanted = JSON.parse(key) as SearchRequest
    let live = true
    let frame = 0
    let pending: Partial<SearchState> = {}
    // Filled as frames arrive; the screen gets a copy of the order once a frame.
    let local: SearchCards = { order: [], byId: new Map() }
    // eslint-disable-next-line react-hooks/set-state-in-effect -- a new search starts from nothing
    setState({ ...initial })
    const flush = () => {
      frame = 0
      if (!live) return
      const next = pending
      pending = {}
      setState((previous) => ({
        ...previous,
        ...next,
        cards: { order: local.order.slice(), byId: local.byId },
      }))
    }
    const schedule = (change: Partial<SearchState>) => {
      pending = { ...pending, ...change }
      if (!frame) frame = requestAnimationFrame(flush)
    }
    const onFrame = (event: SearchFrame) => {
      if (!live) return
      switch (event.type) {
        case 'started':
          schedule({
            phase: 'searching',
            total: event.payload.total,
            indexLag: event.payload.indexLag,
            filtered: event.payload.filtered,
          })
          break
        case 'groups': {
          const { order, byId } = local
          for (const card of event.payload.items) {
            if (!byId.has(card.id)) order.push(card.id)
            byId.set(card.id, card)
          }
          schedule({})
          break
        }
        case 'progress':
          schedule({
            scanned: event.payload.scanned,
            // Unknown until the pool is: keep what `started` said.
            ...(event.payload.total === null
              ? {}
              : { total: event.payload.total }),
            conversations: event.payload.conversations,
            hits: event.payload.hits,
          })
          break
        case 'finished':
          schedule({
            phase: 'done',
            total: event.payload.searchedCount,
            scanned: event.payload.searchedCount,
            conversations: event.payload.conversations,
            hits: event.payload.hits,
            filtered: event.payload.filtered,
            indexLag: event.payload.indexLag,
          })
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
          local = { order: [], byId: new Map() }
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
    openConversationSearch(wanted, onFrame).then(
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

  /** Stops it where it is; the cards so far stay. */
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
