/**
 * Transport for the Agent tools preview (`preview_agent_sources`).
 *
 * The command answers at once with a stream id; the walk's frames follow.
 * Under Tauri they arrive on a `Channel`; through the dev bridge (E2E,
 * `dev:demo`) on a private server-sent event stream at
 * `/previews/{streamId}/events`, which takes one reader and ends after the
 * last frame. Callers get one shape for both.
 *
 * Responsible for: delivering frames in order until a terminal frame or
 * `close()`, and stopping the walk on `close()`. Not responsible for: what
 * the frames mean to a screen (`features/enable-agent/use-agent-preview.ts`).
 */
import { Channel } from '@tauri-apps/api/core'
import { hasTauriGuestApi, resolveDevIpcBridgeUrl } from '../runtime'
import type {
  AgentPreviewRequest,
  PreviewFrame,
  PreviewStarted,
} from '../backend-client/data-kinds'
import { invokeCommand } from './bridge'

/** `error` frame code when the dev bridge's stream drops before the end; the UI translates it. */
export const PREVIEW_CONNECTION_LOST = 'preview-connection-lost'

/** A running preview. `close()` stops the walk; later frames are not delivered. */
export interface AgentPreviewStream extends PreviewStarted {
  close: () => void
}

function isTerminal(frame: PreviewFrame) {
  return (
    frame.type === 'finished' ||
    frame.type === 'cancelled' ||
    frame.type === 'error'
  )
}

/** Asks the backend to stop a walk that has not ended. A refusal (locked, gone) changes nothing. */
function cancel(streamId: string) {
  void invokeCommand('cancel_agent_read', {
    request: { requestIdOrStreamId: streamId },
  }).catch(() => undefined)
}

/**
 * Starts a preview and forwards its frames until a terminal frame or
 * `close()`. Frames can arrive before this promise resolves; they are
 * delivered in order all the same.
 */
export async function openAgentPreview(
  request: AgentPreviewRequest,
  onFrame: (frame: PreviewFrame) => void,
): Promise<AgentPreviewStream> {
  let open = true
  let ended = false
  const deliver = (frame: PreviewFrame) => {
    if (!open || ended) return
    if (isTerminal(frame)) ended = true
    onFrame(frame)
  }

  if (hasTauriGuestApi()) {
    const channel = new Channel<PreviewFrame>()
    channel.onmessage = deliver
    const started = await invokeCommand<PreviewStarted>(
      'preview_agent_sources',
      { request, onEvent: channel },
    )
    return {
      ...started,
      close: () => {
        if (!open) return
        open = false
        if (!ended) cancel(started.streamId)
      },
    }
  }

  const started = await invokeCommand<PreviewStarted>('preview_agent_sources', {
    request,
  })
  const bridgeUrl = resolveDevIpcBridgeUrl()
  const source = new EventSource(
    `${bridgeUrl}/previews/${encodeURIComponent(started.streamId)}/events`,
  )
  source.onmessage = (message) => {
    const frame = JSON.parse(message.data as string) as PreviewFrame
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
      payload: { code: PREVIEW_CONNECTION_LOST },
    })
  }
  return {
    ...started,
    close: () => {
      if (!open) return
      open = false
      source.close()
      // Closing the reader stops the walk on the bridge too; saying so
      // also covers a stream the reader never reached.
      if (!ended) cancel(started.streamId)
    },
  }
}
