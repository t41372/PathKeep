/**
 * AI chat stream event subscription helper.
 *
 * Why this file exists:
 * - `ai_chat_send` returns a run id immediately and then streams tokens, reasoning, tool calls,
 *   and a terminal done/error over the `pathkeep://ai-stream` Tauri event. The UI must subscribe
 *   to that channel to render the answer incrementally without freezing the main thread.
 * - The helper keeps the event wiring small and testable and filters to a single run id, so a
 *   subscriber only sees the chunks for the run it started (multiple runs share one channel).
 *
 * Not responsible for: building chat UI, owning conversation state, or driving the stream
 * (that is the worker's job). The dev HTTP bridge does not deliver Tauri events, so this helper
 * is only live under real Tauri — by design.
 */

import type { AiChatStreamChunk, AiChatStreamEvent } from '../types'

export type AiChatStreamListener = (chunk: AiChatStreamChunk) => void

/**
 * Subscribes to streamed chat chunks for one run id.
 *
 * Only chunks whose envelope `runId` matches `runId` are forwarded, so concurrent runs do not
 * cross-talk. Returns an unsubscribe function; if the Tauri event bridge is unavailable (e.g.
 * browser preview) it degrades to a no-op unsubscribe instead of throwing.
 */
export async function subscribeToAiChatStream(
  runId: string,
  listener: AiChatStreamListener,
) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<AiChatStreamEvent>(
      'pathkeep://ai-stream',
      ({ payload }) => {
        if (payload && payload.runId === runId) listener(payload.chunk)
      },
    )
  } catch {
    return () => {}
  }
}
