/**
 * Reading the repair's counts. They come as decimal strings; a `null` one
 * could not be established and is never read as 0.
 */
import type { RepairPreview } from '@/lib/backend-client/agent-repair'

/** A decimal string from the wire; conversation counts stay far below 2^53. */
export const count = (value: string) => Number(value) || 0

/** A count, or `null` when the backend could not establish it. */
export const maybeCount = (value: string | null) =>
  value === null ? null : count(value)

/**
 * Whether starting over leaves something behind: conversations, or the
 * latest messages of some, or it can't tell. Then a safety copy may hold more.
 */
export function startOverLeavesSomething(preview: RepairPreview) {
  return (
    preview.action === 'startFresh' &&
    (maybeCount(preview.conversationsOnlyInCopy) !== 0 ||
      maybeCount(preview.conversationsRecoveredPartially) !== 0)
  )
}
