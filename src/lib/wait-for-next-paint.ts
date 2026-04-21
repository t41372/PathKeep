/**
 * Paint-first scheduling helper for busy overlays and other trust-critical UI transitions.
 *
 * Why this file exists:
 * - Several routes must let the shell or route overlay repaint before they hand control to heavy desktop work.
 * - Keeping this helper shared prevents each workflow surface from inventing a slightly different "yield to the browser" rule.
 *
 * Main declarations:
 * - `waitForNextPaint`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/ux-principles.md`, especially the Execute paint-first rule.
 * - Callers should use this only around heavyweight work that would otherwise starve the next visible frame.
 */

/**
 * Yields one frame so the next visible loading state can paint before heavy async work begins.
 *
 * The timeout fallback keeps the promise settling in environments where
 * `requestAnimationFrame` is unavailable or throttled more aggressively than a
 * normal desktop shell render.
 */
export function waitForNextPaint() {
  return new Promise<void>((resolve) => {
    if (
      typeof window === 'undefined' ||
      typeof window.requestAnimationFrame !== 'function'
    ) {
      resolve()
      return
    }

    let settled = false

    /**
     * Resolves the current paint wait exactly once, regardless of which
     * scheduling path wins first.
     */
    const finish = () => {
      if (settled) return
      settled = true
      resolve()
    }

    window.requestAnimationFrame(() => finish())
    window.setTimeout(finish, 16)
  })
}
