/**
 * Find in this conversation (`agent-history.md` §4.6): every occurrence in
 * the conversation and its subagents, collapsed parts and titles included,
 * streamed, so a long conversation counts up while it is searched.
 *
 * Responsibilities: one find at a time (a new query or closing stops the
 * one before), the occurrences in order and grouped by where they are
 * drawn, shown once per frame; starting again when the conversation was
 * published anew (`stale_cursor`). Not responsible for: which occurrence
 * is current, scrolling or painting (`find-paint.ts`, `reader.tsx`).
 */
import { useEffect, useState } from 'react'
import type {
  SearchFrame,
  SearchMatch,
} from '@/lib/backend-client/agent-search'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { openConversationFind } from '@/lib/ipc/agent-read-stream'
import { utf8RangeToIndexes } from '@/lib/utf8-range'

/** Where an occurrence is drawn: a record's part, or a subagent's title. */
export function partToken(match: SearchMatch) {
  if (match.role === 'title') return `title:${match.subagentId ?? ''}`
  if (!match.address || match.part === null) return null
  return `${match.address.sourceId}:${match.address.ordinal}:${match.part}`
}

/** The words that matched, as the snippet shows them. */
export function matchedText(match: SearchMatch) {
  const [from, to] = utf8RangeToIndexes(match.text, match.highlight)
  return match.text.slice(from, to)
}

export interface FoundMatches {
  /** In the conversation's order; `items[i].occurrence` is `i + 1`. */
  items: SearchMatch[]
  /** By `partToken`, each in order. */
  byToken: Map<string, SearchMatch[]>
}

export interface FindState {
  phase: 'idle' | 'finding' | 'done' | 'failed'
  matches: FoundMatches
  error: string | null
}

const idle: FindState = {
  phase: 'idle',
  matches: { items: [], byToken: new Map() },
  error: null,
}

export function useFind(id: string, query: string | null, onStale: () => void) {
  const [state, setState] = useState<FindState>(idle)
  const [attempt, setAttempt] = useState(0)

  useEffect(() => {
    if (!query) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- no query, nothing found
      setState(idle)
      return
    }
    let live = true
    let frame = 0
    const items: SearchMatch[] = []
    const byToken = new Map<string, SearchMatch[]>()
    let phase: FindState['phase'] = 'finding'
    let error: string | null = null
    setState({ ...idle, phase: 'finding' })
    const flush = () => {
      frame = 0
      if (!live) return
      setState({ phase, matches: { items: items.slice(), byToken }, error })
    }
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(flush)
    }
    const onFrame = (event: SearchFrame) => {
      if (!live) return
      if (event.type === 'matches') {
        for (const match of event.payload.items) {
          items.push(match)
          const token = partToken(match)
          if (!token) continue
          const list = byToken.get(token)
          if (list) list.push(match)
          else byToken.set(token, [match])
        }
        schedule()
      } else if (event.type === 'finished') {
        phase = 'done'
        schedule()
      } else if (event.type === 'error') {
        if (event.payload.code === 'stale_cursor') {
          onStale()
          setAttempt((n) => n + 1)
          return
        }
        phase = 'failed'
        error = event.payload.code
        schedule()
      }
    }
    let close: (() => void) | null = null
    openConversationFind({ id, query }, onFrame).then(
      (stream) => {
        if (live) close = stream.close
        else stream.close()
      },
      (failure: unknown) => {
        if (!live) return
        phase = 'failed'
        error = commandErrorCode(failure) ?? 'failed'
        schedule()
      },
    )
    return () => {
      live = false
      if (frame) cancelAnimationFrame(frame)
      close?.()
    }
  }, [id, query, attempt, onStale])

  return state
}
