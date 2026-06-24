/**
 * @file use-saved-feedback.ts
 * @description Flash hook for the Settings "Saved" auto-save confirmation chip.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Track a single "just saved" pulse: `flash()` shows the chip for ~1.5s then
 *   hides it. Lives in its own file (not beside the chip component) so the chip
 *   module stays component-only for fast-refresh.
 *
 * ## Not responsible for
 * - Rendering — that is `SettingsSavedChip` in `settings-saved-feedback.tsx`.
 * - Performing the save; callers call `flash()` only after a write lands.
 *
 * ## Performance notes
 * - One timer at a time (cleared on re-flash / unmount). No polling or main-thread work.
 */

import { useCallback, useEffect, useRef, useState } from 'react'

/** How long the chip stays visible after a successful save before it fades out. */
export const SAVED_VISIBLE_MS = 1500

/**
 * Tracks a single "just saved" pulse. `flash()` is called by a section only after
 * a write actually lands, so the chip is a truthful confirmation, never an
 * optimistic one.
 */
export function useSavedFeedback() {
  const [visible, setVisible] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const clear = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const flash = useCallback(() => {
    clear()
    setVisible(true)
    timerRef.current = setTimeout(() => {
      setVisible(false)
      timerRef.current = null
    }, SAVED_VISIBLE_MS)
  }, [clear])

  // Drop any pending timer if the section unmounts mid-pulse.
  useEffect(() => clear, [clear])

  return { visible, flash }
}
