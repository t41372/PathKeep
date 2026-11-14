/**
 * The shortcut that switches to the nth space: ⌘1…⌘9 on macOS, Ctrl+1…Ctrl+9
 * on Windows and Linux, the modifier ⌘K / Ctrl+K uses on each.
 *
 * Unlike ⌘K, Ctrl+1…9 on macOS are not taken: they are the system's "Switch
 * to Desktop N" keys, and when those are on the system has the key before
 * the app sees it.
 *
 * Responsibilities: reading the key and writing its label. Not responsible
 * for: switching (`src/app/shell/current-space.ts`).
 */
import { isMacOsHost } from '@/lib/runtime'

/** How the shortcut for the nth space (1-based) is written. */
export function spaceKeyLabel(n: number) {
  return isMacOsHost() ? `⌘${n}` : `Ctrl+${n}`
}

/** Which space (0-based) a key press asks for, or null when it is not this shortcut. */
export function spaceKeyIndex(event: KeyboardEvent): number | null {
  const mac = isMacOsHost()
  const primary = mac
    ? event.metaKey && !event.ctrlKey
    : event.ctrlKey && !event.metaKey
  if (!primary || event.altKey || event.shiftKey) return null
  const n = Number(event.key)
  return Number.isInteger(n) && n >= 1 && n <= 9 ? n - 1 : null
}
