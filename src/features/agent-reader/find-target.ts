/**
 * The occurrence find is on, for the parts that fold (`agent-history.md`
 * §4.6: an occurrence inside a folded tool call, thought or group of
 * subagents opens it). A fold reads `useReveal(tokens)` and opens itself
 * once whenever the current occurrence moves into one of its parts; the
 * person can close it again.
 */
import { createContext, useContext, useState } from 'react'

export interface FindTarget {
  /** 1-based, in the conversation's order: tells one occurrence from another. */
  occurrence: number
  /** `partToken` of the current occurrence. */
  token: string | null
}

export const FindTargetContext = createContext<FindTarget | null>(null)

/** The `data-part` tokens of record `sourceId:ordinal` for the parts at `indexes`. */
export function partTokens(
  row: { sourceId: string; ordinal: number },
  indexes: number[],
) {
  return indexes.map((index) => `${row.sourceId}:${row.ordinal}:${index}`)
}

/**
 * `[open, setOpen]` for a fold holding `tokens`: open on its own while the
 * current occurrence is in it and has not been seen yet.
 */
export function useReveal(tokens: readonly string[]) {
  const target = useContext(FindTargetContext)
  const [open, setOpen] = useState(false)
  const [seen, setSeen] = useState<number | null>(null)
  const inside =
    target !== null &&
    target.token !== null &&
    tokens.includes(target.token) &&
    seen !== target.occurrence
  if (inside) {
    setSeen(target.occurrence)
    setOpen(true)
  }
  return [open || inside, setOpen] as const
}
