/**
 * Transport for the Agent space's Insights report (`get_agent_insights`).
 *
 * The command answers at once with a stream id; the sections follow. Under
 * Tauri they arrive on a `Channel`; through the dev bridge (E2E,
 * `dev:demo`) on a private server-sent event stream at
 * `/previews/{streamId}/events`, which takes one reader and ends after the
 * last frame. Callers get one shape for both.
 *
 * Responsible for: delivering frames in order until a terminal frame or
 * `close()`, and stopping the report on `close()`. Not responsible for:
 * what the sections mean (`features/agent-insights/use-report.ts`).
 */
import { Channel } from '@tauri-apps/api/core'
import { hasTauriGuestApi, resolveDevIpcBridgeUrl } from '../runtime'
import {
  isTerminalInsightsFrame,
  type InsightsFrame,
  type InsightsQuery,
  type InsightsStarted,
} from '../backend-client/agent-insights'
import { invokeCommand } from './bridge'

/** `error` frame code when the dev bridge's stream drops before the end. */
export const INSIGHTS_CONNECTION_LOST = 'insights-connection-lost'

/** A running report. `close()` stops it; later frames are not delivered. */
export interface AgentInsightsStream extends InsightsStarted {
  close: () => void
}

/** Asks the backend to stop a report that has not ended; a refusal changes nothing. */
function cancel(streamId: string) {
  void invokeCommand('cancel_agent_read', {
    request: { requestIdOrStreamId: streamId },
  }).catch(() => undefined)
}

/**
 * Starts a report and forwards its frames until a terminal frame or
 * `close()`. Frames can arrive before this promise resolves; they are
 * delivered in order all the same.
 */
export async function openAgentInsights(
  request: InsightsQuery,
  onFrame: (frame: InsightsFrame) => void,
): Promise<AgentInsightsStream> {
  let live = true
  let ended = false
  const deliver = (frame: InsightsFrame) => {
    if (!live || ended) return
    if (isTerminalInsightsFrame(frame)) ended = true
    onFrame(frame)
  }

  if (hasTauriGuestApi()) {
    const channel = new Channel<InsightsFrame>()
    channel.onmessage = deliver
    const started = await invokeCommand<InsightsStarted>('get_agent_insights', {
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

  const started = await invokeCommand<InsightsStarted>('get_agent_insights', {
    request,
  })
  const source = new EventSource(
    `${resolveDevIpcBridgeUrl()}/previews/${encodeURIComponent(started.streamId)}/events`,
  )
  source.onmessage = (message) => {
    const frame = JSON.parse(message.data as string) as InsightsFrame
    deliver(frame)
    // The stream ends after the last frame; left open, the browser would
    // connect again and find nothing there.
    if (isTerminalInsightsFrame(frame)) source.close()
  }
  source.onerror = () => {
    if (source.readyState === EventSource.CLOSED || ended) return
    source.close()
    deliver({
      streamId: started.streamId,
      sequence: '',
      type: 'error',
      payload: { code: INSIGHTS_CONNECTION_LOST },
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
