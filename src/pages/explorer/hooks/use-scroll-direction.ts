/**
 * @file use-scroll-direction.ts
 * @description Direction signal for Browse infinite-scroll prefetch.
 * @module pages/explorer/hooks
 *
 * ## Responsibilities
 * - Sample `window.scrollY` per animation frame, classify each delta
 *   as `up` / `down` / `idle` (small deltas), and only flip the
 *   returned direction once a configurable number of consecutive
 *   same-direction samples have arrived (hysteresis).
 *
 * ## Not responsible for
 * - Issuing prefetch requests. The hook is a pure signal — callers
 *   (e.g. `useExplorerInfinitePages`) interpret it.
 * - Per-element scroll containers; the Browse surface scrolls the
 *   document, so this hook listens on `window`.
 *
 * ## Why hysteresis
 * Inertial scroll, trackpad wobble, and momentum scroll all produce
 * brief opposite-direction frames that would otherwise flip the
 * signal back and forth, thrashing the prefetcher. Requiring 4
 * consecutive same-direction frames before flipping smooths the
 * signal without adding perceptible latency (4 frames ≈ 64 ms at
 * 60 fps).
 *
 * ## Performance notes
 * - Single `requestAnimationFrame`-deduped scroll listener.
 * - Allocation-free hot path (fixed-size ring buffer).
 */

import { useEffect, useRef, useState } from 'react'

export type ScrollDirection = 'up' | 'down' | 'idle'

export interface ScrollDirectionOptions {
  /**
   * Minimum scroll delta in CSS pixels for a sample to count as a
   * direction signal. Smaller deltas are treated as `idle`.
   */
  deltaThresholdPx?: number
  /**
   * Number of consecutive same-direction samples required before the
   * returned signal flips.
   */
  hysteresisFrames?: number
}

const DEFAULT_DELTA_THRESHOLD_PX = 4
const DEFAULT_HYSTERESIS_FRAMES = 4

export function useScrollDirection(
  options: ScrollDirectionOptions = {},
): ScrollDirection {
  const {
    deltaThresholdPx = DEFAULT_DELTA_THRESHOLD_PX,
    hysteresisFrames = DEFAULT_HYSTERESIS_FRAMES,
  } = options
  const [direction, setDirection] = useState<ScrollDirection>('idle')

  const lastYRef = useRef(0)
  const sameDirCountRef = useRef(0)
  const lastSampledDirRef = useRef<ScrollDirection>('idle')
  const rafRef = useRef<number | null>(null)

  useEffect(() => {
    if (typeof window === 'undefined') return
    lastYRef.current = window.scrollY

    const sample = () => {
      rafRef.current = null
      const y = window.scrollY
      const delta = y - lastYRef.current
      lastYRef.current = y
      if (Math.abs(delta) < deltaThresholdPx) {
        sameDirCountRef.current = 0
        lastSampledDirRef.current = 'idle'
        return
      }
      const sampled: ScrollDirection = delta > 0 ? 'down' : 'up'
      if (sampled === lastSampledDirRef.current) {
        sameDirCountRef.current += 1
      } else {
        sameDirCountRef.current = 1
        lastSampledDirRef.current = sampled
      }
      if (sameDirCountRef.current >= hysteresisFrames) {
        setDirection((current) => (current === sampled ? current : sampled))
      }
    }

    const onScroll = () => {
      if (rafRef.current !== null) return
      rafRef.current = window.requestAnimationFrame(sample)
    }

    window.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      window.removeEventListener('scroll', onScroll)
      if (rafRef.current !== null) window.cancelAnimationFrame(rafRef.current)
    }
  }, [deltaThresholdPx, hysteresisFrames])

  return direction
}
