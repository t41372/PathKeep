/**
 * The typed reasons the backend gives when AI work on Agent conversations
 * is refused, as a command error's `code` or a search stream's `error`
 * frame (`docs/architecture/desktop-command-surface.md`, "Agent
 * conversation AI access"), and in a failed semantic build's `job.error`.
 *
 * Responsible for: the codes and reading one out of an error. Not
 * responsible for: what a screen says about each
 * (`features/ai-setup/ai-refusals.ts`).
 */
import { commandErrorCode } from '@/lib/ipc/command-error'

export const AI_REFUSAL_CODES = [
  /** The AI switch in Settings › AI. */
  'ai-disabled',
  /** Semantic search in Settings › AI. */
  'semantic-index-disabled',
  /** The assistant in Settings › AI. */
  'assistant-disabled',
  /** The matching opt-in in Settings › Agent conversations. */
  'agent-conversations-access-disabled',
  /** Agent conversations are off in Manage data types. */
  'kind-off',
  /** No enabled embedding model is chosen. */
  'embedding-provider-unavailable',
  /** The model or service changed since the index or the search began. */
  'embedding-provider-changed',
  /** The embeddings service didn't answer. */
  'embedding-service-unavailable',
  /** The in-app model isn't installed or can't be read. */
  'embedding-model-unavailable',
] as const

export type AiRefusalCode = (typeof AI_REFUSAL_CODES)[number]

function isAiRefusalCode(code: string): code is AiRefusalCode {
  return (AI_REFUSAL_CODES as readonly string[]).includes(code)
}

/** The refusal in an error, a frame's code or a job's `error`; `null` for anything else. */
export function aiRefusalCode(errorOrCode: unknown): AiRefusalCode | null {
  const code =
    typeof errorOrCode === 'string'
      ? errorOrCode
      : commandErrorCode(errorOrCode)
  return code !== null && isAiRefusalCode(code) ? code : null
}
