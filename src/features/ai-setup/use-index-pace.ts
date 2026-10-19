/**
 * How fast an index build goes and when it should finish, from the counts the
 * queue reports, plus a count that moves smoothly between those reports.
 *
 * The backend records progress once per batch (one embeddings request, about
 * two seconds apart with LM Studio on a laptop), so the raw count jumps.
 * `useSmoothCount` counts up to each new value over about the gap between
 * reports; it never shows a number the build has not reached.
 *
 * Not responsible for reading the queue or for any copy.
 */
import { useEffect, useRef, useState } from 'react'

/** How far back the speed is measured. */
const WINDOW_MS = 30_000
/** The speed is shown only once it is measured over at least this long. */
const MIN_SPAN_MS = 5_000

export interface IndexPace {
  /** Visits a second over the last half minute. */
  perSecond: number
  /** Seconds left at that speed. */
  secondsLeft: number
}

interface Track {
  key: number | null
  /** `[when the queue answered, done]`, oldest first. */
  points: [number, number][]
}

/**
 * The measured pace of the build `key` at `done` of `total`, where `at` is
 * when the queue reported it (the query's `dataUpdatedAt`); null until there
 * is enough to measure. Another `key` (another build) starts over.
 */
export function useIndexPace(
  key: number | null,
  done: number,
  total: number,
  at: number,
): IndexPace | null {
  const [track, setTrack] = useState<Track>({ key: null, points: [] })
  const last = track.points.at(-1)
  if (track.key !== key || !last || (last[1] !== done && last[0] !== at)) {
    // A new count: remember it (React's "adjust state while rendering").
    const kept = track.key === key ? track.points : []
    setTrack({
      key,
      points: [...kept.filter(([when]) => at - when <= WINDOW_MS), [at, done]],
    })
  }

  const first = track.key === key ? track.points[0] : undefined
  if (key === null || !first || total <= done) return null
  const span = at - first[0]
  const gained = done - first[1]
  if (span < MIN_SPAN_MS || gained <= 0) return null
  const perSecond = gained / (span / 1000)
  return { perSecond, secondsLeft: (total - done) / perSecond }
}

function prefersReducedMotion() {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

/**
 * `value`, counted up smoothly from the previous value over `durationMs`.
 * Shows `value` at once when it goes down, and under reduced motion.
 */
export function useSmoothCount(value: number, durationMs = 1500): number {
  const [shown, setShown] = useState(value)
  const current = useRef(value)

  useEffect(() => {
    const start = current.current
    let frame = 0
    if (value <= start || prefersReducedMotion()) {
      current.current = value
      frame = requestAnimationFrame(() => setShown(value))
      return () => cancelAnimationFrame(frame)
    }
    const began = performance.now()
    const step = (now: number) => {
      const t = Math.min(1, (now - began) / durationMs)
      const next = Math.round(start + (value - start) * t)
      current.current = next
      setShown(next)
      if (t < 1) frame = requestAnimationFrame(step)
    }
    frame = requestAnimationFrame(step)
    return () => cancelAnimationFrame(frame)
  }, [value, durationMs])

  return Math.min(shown, value)
}
