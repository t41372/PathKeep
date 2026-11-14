/**
 * The space the user was last in, kept on this computer the way the theme
 * is, so `#/` and the next launch open it again, and shared screens (Ask,
 * Settings) keep showing its pages in the rail.
 *
 * Responsibilities: reading and writing that one preference, falling back
 * to the first space that is on when nothing is stored, storage is blocked,
 * or the stored space is no longer on.
 * Not responsible for: following the URL (`src/app/shell/current-space.ts`).
 */
import { spacesOn, type Space } from './registry'

const STORAGE_KEY = 'pathkeep-space'

export function readLastSpace(): Space {
  const on = spacesOn()
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    const space = on.find((item) => item.id === stored)
    if (space) return space
  } catch {
    // Fall through to the first space.
  }
  return on[0]
}

export function rememberSpace(id: string) {
  try {
    localStorage.setItem(STORAGE_KEY, id)
  } catch {
    // The choice still holds for this window.
  }
}
