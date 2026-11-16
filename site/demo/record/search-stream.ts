/**
 * Recording build: the app's own search stream, with each search's request,
 * its start and the events it streamed logged.
 */
import * as original from '@/lib/ipc/search-stream'
import type { SearchEvent } from '@/lib/backend-client/search'
import { logSearch } from './log'

export const SEARCH_CONNECTION_LOST = original.SEARCH_CONNECTION_LOST

export type SearchStream = original.SearchStream

export const openSearchStream: typeof original.openSearchStream = async (
  request,
  onEvent,
) => {
  // Events can arrive before the stream is returned; keep those for the log.
  const early: SearchEvent[] = []
  let events: SearchEvent[] | null = null
  const stream = await original.openSearchStream(request, (event) => {
    ;(events ?? early).push(event)
    onEvent(event)
  })
  const { close: _close, ...started } = stream
  events = logSearch(request, started)
  events.push(...early)
  return stream
}
