/**
 * AI chat stream event subscription helper.
 *
 * Why this file exists:
 * - `ai_chat_send` returns a run id and then streams tokens, reasoning, tool calls and a terminal
 *   done/error on `pathkeep://ai-stream` (a Tauri event in the app; the dev bridge's `/events`
 *   in E2E and `dev:demo`). Runs share the channel; each event names its run.
 * - A run can be over before `ai_chat_send` returns (a quick answer, an address that refuses the
 *   connection), so callers listen first and pick their run's events once they know its id.
 *
 * Not responsible for: building chat UI, owning conversation state, or driving the stream
 * (that is the worker's job).
 */

import type { AiChatStreamEvent } from '../types'
import { listenDesktopEvent } from './events'

/** Matches `vault_core::AI_CHAT_STREAM_EVENT`. */
const AI_CHAT_STREAM_EVENT = 'pathkeep://ai-stream'

/** Listens to every run's events until the returned function is called. */
export function listenToAiChatStream(
  listener: (event: AiChatStreamEvent) => void,
) {
  return listenDesktopEvent<AiChatStreamEvent | null>(
    AI_CHAT_STREAM_EVENT,
    (event) => {
      if (event) listener(event)
    },
  )
}
