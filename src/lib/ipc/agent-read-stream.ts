/**
 * Transport for the Agent space's streamed reads: Records search
 * (`search_agent_conversations`, by words or by meaning), the reader's find
 * (`find_in_agent_conversation`) and the Insights report
 * (`get_agent_insights`).
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
import type {
  InsightsFrame,
  InsightsQuery,
} from '../backend-client/agent-insights'
import type {
  FindRequest,
  SearchFrame,
  SearchRequest,
  SemanticSearchFrame,
  SemanticSearchRequest,
  StreamStarted,
} from '../backend-client/agent-search'
import { invokeCommand } from './bridge'

/** `error` frame code when the dev bridge's stream drops before the end. */
export const STREAM_CONNECTION_LOST = 'stream-connection-lost'

/** A running read. `close()` stops it; later frames are not delivered. */
export interface AgentReadStream extends StreamStarted {
  close: () => void
}

/** What the frames of every read share. */
interface ReadFrame {
  streamId: string
  sequence: string
  type: string
}

/** The frame a dropped bridge stream ends with; every read has this shape of `error`. */
interface LostFrame extends ReadFrame {
  type: 'error'
  payload: { code: string }
}

/** `finished`, `cancelled` and `error` end every read. */
function isTerminal(frame: ReadFrame) {
  return (
    frame.type === 'finished' ||
    frame.type === 'cancelled' ||
    frame.type === 'error'
  )
}

function cancel(streamId: string) {
  void invokeCommand('cancel_agent_read', {
    request: { requestIdOrStreamId: streamId },
  }).catch(() => undefined)
}

/**
 * Starts a read and forwards its frames until a terminal frame or
 * `close()`. Frames can arrive before this promise resolves; they are
 * delivered in order all the same.
 */
async function open<Frame extends ReadFrame>(
  command:
    | 'search_agent_conversations'
    | 'find_in_agent_conversation'
    | 'get_agent_insights',
  request: SearchRequest | FindRequest | InsightsQuery,
  onFrame: (frame: Frame | LostFrame) => void,
): Promise<AgentReadStream> {
  let live = true
  let ended = false
  const deliver = (frame: Frame | LostFrame) => {
    if (!live || ended) return
    if (isTerminal(frame)) ended = true
    onFrame(frame)
  }

  if (hasTauriGuestApi()) {
    const channel = new Channel<Frame>()
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
    const frame = JSON.parse(message.data as string) as Frame
    deliver(frame)
    // The stream ends after the last frame; left open, the browser would
    // connect again and find nothing there.
    if (isTerminal(frame)) source.close()
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
  return open<SearchFrame>('search_agent_conversations', request, onFrame)
}

/**
 * One page of a search by meaning: up to 100 passages, closest first. The
 * page's `finished` names the cursor of the next one.
 */
export function openSemanticSearch(
  request: SemanticSearchRequest,
  onFrame: (frame: SemanticSearchFrame) => void,
) {
  return open<SemanticSearchFrame>(
    'search_agent_conversations',
    request,
    onFrame,
  )
}

/** Finds every occurrence in one conversation and its subagents. */
export function openConversationFind(
  request: FindRequest,
  onFrame: (frame: SearchFrame) => void,
) {
  return open<SearchFrame>('find_in_agent_conversation', request, onFrame)
}

/** The Insights report: its sections, then the year, as each is ready. */
export function openAgentInsights(
  request: InsightsQuery,
  onFrame: (frame: InsightsFrame) => void,
) {
  return open<InsightsFrame>('get_agent_insights', request, onFrame)
}
