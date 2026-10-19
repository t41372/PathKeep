/**
 * Transport for streamed search sessions.
 *
 * Why this file exists:
 * - `start_search` returns a session id at once and then pushes ranking steps,
 *   counts and index progress. Under Tauri they arrive on a `Channel`; through
 *   the dev bridge (E2E, `dev:demo`) they arrive as server-sent events on
 *   `/searches/{sessionId}/events`. Callers get one shape for both.
 *
 * Not responsible for: holding results or deciding what the UI shows; see
 * `features/history/use-search-stream.ts`.
 */

import { Channel } from '@tauri-apps/api/core'
import { hasTauriGuestApi, resolveDevIpcBridgeUrl } from '../runtime'
import type {
  SearchEvent,
  SearchRequest,
  SearchStarted,
} from '../backend-client/search'
import { invokeCommand } from './bridge'

/** Error event data when the dev bridge's event stream drops; the UI translates it. */
export const SEARCH_CONNECTION_LOST = 'search-connection-lost'

/** A running search. `close()` stops it; later events are not delivered. */
export interface SearchStream extends SearchStarted {
  close: () => void
}

/**
 * Starts a search and forwards its events until `close()` or a terminal event.
 * Events can arrive before this promise resolves; they are buffered by the
 * transport and delivered in order.
 */
export async function openSearchStream(
  request: SearchRequest,
  onEvent: (event: SearchEvent) => void,
): Promise<SearchStream> {
  let open = true
  const deliver = (event: SearchEvent) => {
    if (open) onEvent(event)
  }

  if (hasTauriGuestApi()) {
    const channel = new Channel<SearchEvent>()
    channel.onmessage = deliver
    const started = await invokeCommand<SearchStarted>('start_search', {
      request,
      onEvent: channel,
    })
    return {
      ...started,
      close: () => {
        if (!open) return
        open = false
        void invokeCommand('cancel_search', {
          sessionId: started.sessionId,
        }).catch(() => undefined)
      },
    }
  }

  const started = await invokeCommand<SearchStarted>('start_search', {
    request,
  })
  const bridgeUrl = resolveDevIpcBridgeUrl()
  // Closing the event stream cancels the session on the bridge side.
  const source = new EventSource(
    `${bridgeUrl}/searches/${encodeURIComponent(started.sessionId)}/events`,
  )
  source.onmessage = (message) => {
    const event = JSON.parse(message.data as string) as SearchEvent
    deliver(event)
    if (event.type === 'cancelled' || event.type === 'error') source.close()
  }
  source.onerror = () => {
    if (source.readyState === EventSource.CLOSED) return
    source.close()
    deliver({ type: 'error', data: SEARCH_CONNECTION_LOST })
  }
  return {
    ...started,
    close: () => {
      open = false
      source.close()
    },
  }
}
