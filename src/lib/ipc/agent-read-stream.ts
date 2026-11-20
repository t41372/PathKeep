/**
 * Transport for the Agent space's streamed reads: Records search
 * (`search_agent_conversations`) and the reader's find
 * (`find_in_agent_conversation`).
 *
 * The command answers at once with a stream id; the frames follow. Under
 * Tauri they arrive on a `Channel`; through the dev bridge (E2E,
 * `dev:demo`) on a private server-sent event stream at
 * `/previews/{streamId}/events`, which takes one reader and ends after the
 * last frame. Callers get one shape for both.
 *
 * Responsible for: delivering frames in order until a terminal frame or
 * `close()`, and stopping the read on `close()`. Not responsible for: what
 * the frames mean to a screen.
 */
import { Channel } from '@tauri-apps/api/core'
import { hasTauriGuestApi, resolveDevIpcBridgeUrl } from '../runtime'
import {
  isTerminalFrame,
  type FindRequest,
  type SearchFrame,
  type SearchRequest,
  type StreamStarted,
} from '../backend-client/agent-search'
import { invokeCommand } from './bridge'

/** `error` frame code when the dev bridge's stream drops before the end. */
export const STREAM_CONNECTION_LOST = 'stream-connection-lost'

/** A running read. `close()` stops it; later frames are not delivered. */
export interface AgentReadStream extends StreamStarted {
  close: () => void
}

function cancel(streamId: string) {
  void invokeCommand('cancel_agent_read', {
    request: { requestIdOrStreamId: streamId },
  }).catch(() => undefined)
}

async function open(
  command: 'search_agent_conversations' | 'find_in_agent_conversation',
  request: SearchRequest | FindRequest,
  onFrame: (frame: SearchFrame) => void,
): Promise<AgentReadStream> {
  let live = true
  let ended = false
  const deliver = (frame: SearchFrame) => {
    if (!live || ended) return
    if (isTerminalFrame(frame)) ended = true
    onFrame(frame)
  }

  if (hasTauriGuestApi()) {
    const channel = new Channel<SearchFrame>()
    channel.onmessage = deliver
    const started = await invokeCommand<StreamStarted>(command, {
      request,
      onEvent: channel,
    })
    return {
      ...started,
      close: () => {
        if (!live) return
        live = false
        if (!ended) cancel(started.streamId)
      },
    }
  }

  const started = await invokeCommand<StreamStarted>(command, { request })
  const source = new EventSource(
    `${resolveDevIpcBridgeUrl()}/previews/${encodeURIComponent(started.streamId)}/events`,
  )
  source.onmessage = (message) => {
    const frame = JSON.parse(message.data as string) as SearchFrame
    deliver(frame)
    // The stream ends after the last frame; left open, the browser would
    // connect again and find nothing there.
    if (isTerminalFrame(frame)) source.close()
  }
  source.onerror = () => {
    if (source.readyState === EventSource.CLOSED || ended) return
    source.close()
    deliver({
      streamId: started.streamId,
      sequence: '',
      type: 'error',
      payload: { code: STREAM_CONNECTION_LOST },
    })
  }
  return {
    ...started,
    close: () => {
      if (!live) return
      live = false
      source.close()
      if (!ended) cancel(started.streamId)
    },
  }
}

/** Searches saved conversations; every match streams, nothing is capped. */
export function openConversationSearch(
  request: SearchRequest,
  onFrame: (frame: SearchFrame) => void,
) {
  return open('search_agent_conversations', request, onFrame)
}

/** Finds every occurrence in one conversation and its subagents. */
export function openConversationFind(
  request: FindRequest,
  onFrame: (frame: SearchFrame) => void,
) {
  return open('find_in_agent_conversation', request, onFrame)
}
