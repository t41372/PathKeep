/**
 * Viewport-gated lazy mount wrapper for Intelligence hub secondary sections.
 *
 * ## Responsibilities
 * - Defer mounting a heavy secondary section's real DOM until it is near the
 *   viewport, rendering a lightweight skeleton placeholder until then.
 * - Drive the near-viewport decision with an `IntersectionObserver` using a
 *   generous `rootMargin` so a section starts mounting shortly BEFORE it
 *   scrolls into view — the reveal feels instant, never a blank wait.
 * - Reserve layout space via the placeholder so the document height stays
 *   honest and the scrollbar does not jump as sections swap in.
 *
 * ## Not responsible for
 * - Fetching section data. The staged overview hook batch-prefetches every
 *   secondary section into cache when `secondaryReady` flips, so a section that
 *   mounts here reads warm cache. This component only gates DOM render.
 * - Deciding whether the data is ready. The caller combines this viewport
 *   gate with its own data-readiness check (it passes its own skeleton).
 * - Recycling / unmounting on scroll-away. Once a section mounts it stays
 *   mounted; the hub has a bounded set (~12) of secondary sections, so there
 *   is no unbounded DOM growth to recycle. CSS `content-visibility: auto` on
 *   the wrapper (hub.css) keeps off-screen mounted sections cheap to lay out
 *   and paint; the row-major grid means it never reorders sibling cards.
 *
 * ## Dependencies
 * - `IntersectionObserver` (browser). jsdom does not provide it; we mount
 *   immediately in that case so server/test environments render everything.
 *
 * ## Performance notes
 * - One observer per section, disconnected on first intersection and on
 *   unmount. With ~12 sections the observer overhead is negligible, far below
 *   the per-section render cost we are deferring.
 */

import { type ReactNode, useEffect, useRef, useState } from 'react'

/**
 * Buffer applied to the `IntersectionObserver` rootMargin so a section starts
 * mounting/painting before it actually scrolls into view. 400px is roughly
 * half a viewport on the target box — enough head-start that the warm-cache
 * render lands before the user reaches the section, without mounting the whole
 * page at once.
 */
export const LAZY_SECTION_ROOT_MARGIN = '400px 0px'

interface LazySectionProps {
  /**
   * Placeholder rendered while the section is neither near the viewport nor
   * otherwise forced to mount. Typically the same skeleton the caller shows
   * while data warms, so the not-yet-near and not-yet-ready states look
   * identical and there is no layout shift between them.
   */
  skeleton: ReactNode
  /** The real section node, mounted only once near the viewport. */
  children: ReactNode
  /**
   * Escape hatch for callers that have already decided the section must render
   * now (e.g. its data is cached and ready up-front). When `true` the children
   * mount immediately and no observer is installed. Defaults to `false`.
   */
  forceMount?: boolean
}

/**
 * Wraps a heavy hub section so its real DOM mounts only when it scrolls near
 * the viewport. Until then the caller-provided skeleton holds the space.
 *
 * Why it exists: rendering all ~12 secondary intelligence sections eagerly is
 * janky on the target 4-core/8GB box with 14.4M-visit archives. This keeps
 * every section visible-on-scroll (no expand/collapse) while bounding the
 * up-front render cost to what is actually near the viewport.
 *
 * a11y: the skeleton is inert decoration and must not trap focus; the mounted
 * children behave as normal interactive content.
 */
export function LazySection({
  skeleton,
  children,
  forceMount = false,
}: LazySectionProps) {
  const ref = useRef<HTMLDivElement | null>(null)
  // Mount up front when we cannot (or must not) lazily gate: the caller forced
  // it, or IntersectionObserver is unavailable (jsdom, SSR, very old browsers)
  // — in which case there is no way to detect near-viewport, so render
  // everything rather than leave a section stuck behind its skeleton.
  const [mounted, setMounted] = useState(
    () => forceMount || typeof IntersectionObserver === 'undefined',
  )

  useEffect(() => {
    // Already mounted (forced, no-IO, or a prior intersection): nothing to gate.
    if (mounted) return
    // We always render our own ref'd wrapper div, so by effect-commit time
    // `ref.current` is guaranteed to be that element — no null guard needed.
    const node = ref.current as HTMLDivElement
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          // Once mounted we never go back; disconnect so the observer does no
          // further work for this now-permanent section.
          setMounted(true)
          observer.disconnect()
        }
      },
      { root: null, rootMargin: LAZY_SECTION_ROOT_MARGIN, threshold: 0 },
    )
    observer.observe(node)
    return () => observer.disconnect()
  }, [mounted])

  return (
    <div className="intelligence-lazy-section" ref={ref}>
      {mounted ? children : skeleton}
    </div>
  )
}
