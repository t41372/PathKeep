/**
 * The shortcut that jumps to the nth space: ⌘1…⌘9 on macOS, Ctrl+1…Ctrl+9 on
 * Windows and Linux, the same modifier as ⌘K / Ctrl+K.
 *
 * Not ⌃ on macOS: ⌃1…⌃9 are the system's "Switch to Desktop N" keys, and when
 * they are on the system takes them before the app sees the key.
 */
import { isMacOsHost } from '@/lib/runtime'

export const spaceKeyLabel = (n: number) => (isMacOsHost() ? `⌘${n}` : `Ctrl+${n}`)

/** Which space (0-based) a key press asks for, or null when it isn't this shortcut. */
export function spaceKeyIndex(event: KeyboardEvent): number | null {
  const primary = isMacOsHost() ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey
  if (!primary || event.altKey || event.shiftKey) return null
  const n = Number(event.key)
  return Number.isInteger(n) && n >= 1 && n <= 9 ? n - 1 : null
}
