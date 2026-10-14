/**
 * How full the conversation is against the model's context, as the next
 * request would send it.
 *
 * Responsibilities: the same estimate the backend makes before it sends
 * (`estimate_request_tokens` in `agent_harness.rs`), so the meter and the
 * backend's "does not fit" agree. Not responsible for asking the service for
 * the context length (`ai_chat_context`) or rendering.
 */
import type { ChatTurn } from './chat-model'

/**
 * About how many tokens `text` is: Chinese, Japanese and Korean about one per
 * character, other text about four bytes per token. No tokenizer; a model's
 * own counts are better, and the backend refuses what does not fit by the
 * same rule.
 */
export function estimateTokens(text: string) {
  let wide = 0
  let bytes = 0
  for (const char of text) {
    const code = char.codePointAt(0) ?? 0
    if (
      (code >= 0x3040 && code <= 0x30ff) ||
      (code >= 0x3400 && code <= 0x9fff) ||
      (code >= 0xac00 && code <= 0xd7af) ||
      (code >= 0xf900 && code <= 0xfaff)
    )
      wide += 1
    else bytes += code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4
  }
  return wide + Math.ceil(bytes / 4)
}

/** Tokens the next request takes: what every request carries, the conversation and the draft. */
export function nextRequestTokens(
  fixedTokens: number,
  turns: readonly ChatTurn[],
  draft: string,
) {
  // Each message carries a few tokens of role and framing, as on the backend.
  const messages = turns
    .filter((turn) => turn.role === 'user' || turn.content)
    .reduce((sum, turn) => sum + 4 + estimateTokens(turn.content), 0)
  return fixedTokens + messages + (draft.trim() ? 4 + estimateTokens(draft) : 0)
}

export type ContextLevel = 'unknown' | 'ok' | 'near' | 'over'

/** Near from 80 %, over past the context (the backend then refuses to send). */
export function contextLevel(used: number, contextLength: number | null) {
  if (!contextLength) return 'unknown'
  if (used > contextLength) return 'over'
  return used >= contextLength * 0.8 ? 'near' : 'ok'
}
