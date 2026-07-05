/**
 * @file use-stick-to-bottom.ts
 * @description Reusable "stick-to-bottom" scroll model for streaming surfaces.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Track, per scroll container, whether the user is currently parked at (or within a small
 *   threshold of) the bottom. While that holds, callers should follow new content to the bottom;
 *   the instant the user scrolls UP by any meaningful amount, following stops and stays off until
 *   they scroll back down to the bottom themselves.
 * - Expose `scrollToBottom()` so a caller can pin to bottom ONLY when `stickToBottom` is true
 *   (mid-stream) or unconditionally on a deliberate boundary (a new turn).
 *
 * ## Not responsible for
 * - Deciding WHEN to follow. The caller wires `scrollToBottom()` into its own content-change /
 *   new-turn effects; this hook only answers "is the user at the bottom right now?".
 * - Smooth vs instant scrolling: it writes `scrollTop` directly (instant), which inherently honors
 *   `prefers-reduced-motion` (no animated scroll is ever started).
 *
 * ## Why this exists
 * The old streaming-follow logic re-read a per-chunk near-bottom snapshot and still yanked back any
 * scroll-up smaller than its window, so the view felt locked while reasoning/answer streamed in. A
 * persistent `stickToBottom` flag — flipped by an rAF-deduped scroll listener — lets the user scroll
 * up and STAY there during streaming, then resume auto-follow only when they return to the bottom.
 *
 * ## Performance notes
 * - One `scroll` listener per container, rAF-deduped so we read layout at most once per frame
 *   (cheap; no main-thread jank even under a fast streaming flush).
 * - The listener is `passive` and cleaned up on unmount / ref change.
 */

import { useCallback, useEffect, useRef, useState } from 'react'

/**
 * Pixels of slack from the true bottom that still counts as "at the bottom". Sub-pixel rounding and
 * fractional `clientHeight`/`scrollHeight` mean an exact `=== 0` check would flicker off following
 * even when the user is visually pinned, so we allow a small tolerance.
 */
export const STICK_TO_BOTTOM_THRESHOLD_PX = 24

export interface StickToBottom {
  /**
   * True while the user is essentially at the bottom of the container (within
   * {@link STICK_TO_BOTTOM_THRESHOLD_PX}). Auto-follow callers should only pin to bottom when this
   * is true.
   */
  stickToBottom: boolean
  /** Imperatively jump the container to its bottom and mark it stuck again. */
  scrollToBottom: () => void
}

/**
 * Wire stick-to-bottom tracking onto a scroll container ref.
 *
 * @param ref The scroll container whose `scrollTop`/`scrollHeight`/`clientHeight` drive the model.
 * @param attachKey Optional value that, when changed, forces the scroll listener to re-attach. Pass
 *   this when the scrolled element mounts/unmounts behind a stable ref object (e.g. a panel body
 *   that only renders while expanded) — the ref identity alone never changes, so without a key the
 *   listener would not bind to the freshly-mounted node. Containers that are always mounted can omit
 *   it.
 */
export function useStickToBottom(
  ref: React.RefObject<HTMLElement | null>,
  attachKey?: unknown,
): StickToBottom {
  // Default true: a freshly-mounted conversation surface starts pinned to the latest content.
  const [stickToBottom, setStickToBottom] = useState(true)
  // Mirror the flag in a ref so `scrollToBottom` can read the current value without being recreated
  // (its identity must stay stable so caller effects don't re-run every render).
  const stickRef = useRef(true)
  const rafRef = useRef<number | null>(null)

  const setStick = useCallback((next: boolean) => {
    stickRef.current = next
    setStickToBottom((current) => (current === next ? current : next))
  }, [])

  const scrollToBottom = useCallback(() => {
    const node = ref.current
    if (!node) return
    node.scrollTop = node.scrollHeight
    setStick(true)
  }, [ref, setStick])

  useEffect(() => {
    const node = ref.current
    if (!node) return

    const sample = () => {
      rafRef.current = null
      const distanceFromBottom =
        node.scrollHeight - node.scrollTop - node.clientHeight
      setStick(distanceFromBottom <= STICK_TO_BOTTOM_THRESHOLD_PX)
    }

    const onScroll = () => {
      // rAF-dedup: coalesce a burst of scroll events into a single layout read per frame so a fast
      // streaming flush never thrashes the main thread.
      if (rafRef.current !== null) return
      rafRef.current = requestAnimationFrame(sample)
    }

    node.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      node.removeEventListener('scroll', onScroll)
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current)
      rafRef.current = null
    }
    // `attachKey` is intentionally in the dep list so the listener re-binds when a conditionally
    // rendered scroll node (re)mounts behind a stable ref. eslint-disable-next-line is unnecessary:
    // the other deps are stable.
  }, [ref, setStick, attachKey])

  return { stickToBottom, scrollToBottom }
}
