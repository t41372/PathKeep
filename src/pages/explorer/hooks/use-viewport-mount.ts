/**
 * @file use-viewport-mount.ts
 * @description Viewport-driven render gating for Browse day blocks.
 * @module pages/explorer/hooks
 *
 * ## Responsibilities
 * - Track whether the wrapped element is intersecting the viewport
 *   (with a generous `rootMargin` buffer so content is mounted just
 *   before it scrolls into view).
 * - Capture the rendered height each time the element is in view so
 *   the parent can render a height-preserving placeholder when it
 *   later goes out of view — keeping scroll position stable and the
 *   total document height honest.
 *
 * ## Not responsible for
 * - Deciding what to render. The caller chooses whether to swap real
 *   content for a placeholder when `inView === false`.
 * - Cross-day coordination. Each subscriber owns its own observer.
 * - Reusing observers (one per consumer); BROWSE-VIRT typically
 *   wraps 100s of days, not 10 000s, so the observer overhead is
 *   bounded.
 *
 * ## Why this exists
 * `PaperContactSheet` previously rendered every infinite-scrolled
 * day into the DOM at all times. The
 * `browse-virt-spike-2026-05-25` measurement showed list mode hits
 * ~31 k DOM nodes at the 5 000-row cap and cards mode hits ~71 k —
 * exactly the regime where Chrome's compositor / style recalc go
 * non-linear on the target 4-core / 8 GB box. This hook is the
 * lightest possible recycler that bounds DOM cost by *visible* days
 * rather than *loaded* days, while preserving CSS `position: sticky`
 * for the day separator (which `transform`-based virtualisers
 * (e.g. `@tanstack/react-virtual`) would have broken).
 *
 * ## Performance notes
 * - Default `rootMargin` is one screen above and below so a fast
 *   scroller doesn't see content pop in.
 * - jsdom does not fire IntersectionObserver entries, so test
 *   environments stay in the `initialInView` value and render
 *   everything — pre-virt tests keep passing without modification.
 */

import { useEffect, useRef, useState } from 'react'

export interface ViewportMountOptions {
  /** IntersectionObserver `rootMargin`. Defaults to one screen of buffer. */
  rootMargin?: string
  /**
   * Initial `inView` value before the IntersectionObserver reports
   * its first entry. Defaults to `true` so day blocks render
   * synchronously on first mount and tests / SSR get the full tree.
   */
  initialInView?: boolean
  /**
   * When `true`, the hook returns `{ inView: true }` without
   * installing an IntersectionObserver. Callers that have already
   * decided they don't want viewport-driven recycling (e.g.
   * `<PaperContactSheet disableVirtualization />` in spike harnesses
   * and unit tests) use this to avoid spinning up 100+ wasted
   * observers per render. The hook still returns a stable `ref` and
   * a `measuredHeight` of `null` so consumers can keep the same
   * branching shape.
   */
  skip?: boolean
}

export interface ViewportMountState<TElement extends HTMLElement> {
  /** Ref the consumer attaches to its outer element. */
  ref: React.RefObject<TElement | null>
  /** True when the element is intersecting the viewport ± buffer. */
  inView: boolean
  /**
   * Last measured height of the wrapped element while it was in view.
   * Consumers use this to render a placeholder of equivalent height
   * after the element goes out of view, preserving scroll position.
   * Falls back to `null` until the first measurement.
   */
  measuredHeight: number | null
}

const DEFAULT_ROOT_MARGIN = '100% 0px'

export function useViewportMount<TElement extends HTMLElement = HTMLDivElement>(
  options: ViewportMountOptions = {},
): ViewportMountState<TElement> {
  const {
    rootMargin = DEFAULT_ROOT_MARGIN,
    initialInView = true,
    skip = false,
  } = options
  const ref = useRef<TElement | null>(null)
  const [inView, setInView] = useState(initialInView)
  const [measuredHeight, setMeasuredHeight] = useState<number | null>(null)

  useEffect(() => {
    if (skip) return
    const node = ref.current
    if (!node) return
    if (typeof IntersectionObserver === 'undefined') return
    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[0]
        if (!entry) return
        if (entry.isIntersecting) {
          setInView(true)
        } else {
          // Capture last-known height *before* the parent unmounts
          // the content, so the placeholder can reserve the right
          // amount of scroll-space and the user's scroll position
          // doesn't jump.
          const height = node.getBoundingClientRect().height
          if (height > 0) setMeasuredHeight(height)
          setInView(false)
        }
      },
      { rootMargin, threshold: 0 },
    )
    observer.observe(node)
    return () => observer.disconnect()
  }, [rootMargin, skip])

  // While the element is in view, opportunistically refresh the
  // measured height — the day content height changes with view mode
  // toggles, lazy image loads, and day-insights fetches, and a stale
  // measurement would create the wrong placeholder size on next
  // recycle. `measuredHeight` is intentionally NOT in the dep list —
  // the effect only needs to fire when `inView` flips; re-firing on
  // every height-state change would cause an infinite update loop.
  useEffect(() => {
    if (skip || !inView) return
    const node = ref.current
    if (!node) return
    const height = node.getBoundingClientRect().height
    if (height > 0) {
      setMeasuredHeight(height)
    }
  }, [inView, skip])

  return { ref, inView: skip ? true : inView, measuredHeight }
}
