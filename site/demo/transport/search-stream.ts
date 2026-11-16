/**
 * Stands in for `src/lib/ipc/search-stream.ts` in the demo build: a search
 * plays back its events (recorded, or the demo's own), one per frame, the
 * way the real backend streams them.
 */
import type * as Original from '@/lib/ipc/search-stream'
import { searchFor } from '../backend'

export const SEARCH_CONNECTION_LOST: typeof Original.SEARCH_CONNECTION_LOST =
  'search-connection-lost'

export type SearchStream = Original.SearchStream

export const openSearchStream: typeof Original.openSearchStream = async (
  request,
  onEvent,
) => {
  const search = searchFor(request)
  let open = true
  const events = [...search.events]
  const next = () => {
    const event = events.shift()
    if (!open || !event) return
    onEvent(event)
    requestAnimationFrame(next)
  }
  requestAnimationFrame(next)
  return {
    ...search.started,
    close: () => {
      open = false
    },
  }
}
