/**
 * React hooks for Core Intelligence data fetching.
 *
 * Why this file exists:
 * - Provides a consistent data-fetching pattern for Core Intelligence pages.
 * - Wraps the IPC API calls with loading/error/data state management.
 * - Uses a simple useEffect + useState pattern (no external deps like SWR).
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md`
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { describeError } from '../errors'
import type { DateRange, TimeRangePreset } from './types'

// ---------------------------------------------------------------------------
// Time range helpers
// ---------------------------------------------------------------------------

/**
 * Lower bound for the route-level all-time preset.
 *
 * The backend command contract still accepts concrete `DateRange` payloads, so
 * the UI maps all-time to a deliberately broad browser-history window instead
 * of introducing a second transport shape.
 */
export const ALL_TIME_DATE_RANGE_START = '1900-01-01'

/**
 * Computes a concrete DateRange from a preset relative to today.
 *
 * All dates are formatted as ISO date strings (YYYY-MM-DD).
 */
export function dateRangeFromPreset(preset: TimeRangePreset): DateRange {
  const now = new Date()
  const end = formatDate(now)

  switch (preset) {
    case 'day': {
      return { start: end, end }
    }
    case 'week': {
      const start = new Date(now)
      start.setDate(start.getDate() - 7)
      return { start: formatDate(start), end }
    }
    case 'month': {
      const start = new Date(now)
      start.setMonth(start.getMonth() - 1)
      return { start: formatDate(start), end }
    }
    case 'quarter': {
      const start = new Date(now)
      start.setMonth(start.getMonth() - 3)
      return { start: formatDate(start), end }
    }
    case 'year': {
      const start = new Date(now)
      start.setFullYear(start.getFullYear() - 1)
      return { start: formatDate(start), end }
    }
    case 'all': {
      return { start: ALL_TIME_DATE_RANGE_START, end }
    }
    case 'custom':
      // Custom requires explicit start/end — default to month as fallback
      return dateRangeFromPreset('month')
  }
}

/**
 * Builds an inclusive calendar-year range for one local year.
 *
 * This differs from the rolling `year` preset above, which intentionally means
 * "the last 12 months" for the Intelligence route time bar.
 */
export function dateRangeForCalendarYear(year: number): DateRange {
  return {
    start: `${year}-01-01`,
    end: `${year}-12-31`,
  }
}

function formatDate(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

// ---------------------------------------------------------------------------
// Generic async data hook
// ---------------------------------------------------------------------------

/**
 * Names the route-facing async read state returned by Core Intelligence hooks.
 *
 * Exporting the shape keeps mutation and declaration checkers from inferring an
 * unnamed private type through helper hooks that compose `useAsyncData()`.
 */
export interface AsyncState<T> {
  data: T | null
  loading: boolean
  error: string | null
}

interface AsyncDataOptions<T> {
  getCached?: () => T | null
}

/**
 * Generic hook that wraps an async IPC call with loading/error/data state.
 *
 * Re-fetches whenever `deps` changes (shallow JSON comparison).
 * Returns a `refresh` callback for manual re-fetch.
 */
export function useAsyncData<T>(
  fetcher: () => Promise<T>,
  deps: unknown[],
  options?: AsyncDataOptions<T>,
): AsyncState<T> & { refresh: () => void } {
  const readCached = useCallback(
    () => options?.getCached?.() ?? null,
    [options],
  )
  const [state, setState] = useState<AsyncState<T>>({
    data: readCached(),
    loading: readCached() === null,
    error: null,
  })

  const depsRef = useRef<string | null>(null)
  const requestIdRef = useRef(0)
  const fetcherRef = useRef(fetcher)
  const depsJson = JSON.stringify(deps)

  useEffect(() => {
    fetcherRef.current = fetcher
  }, [fetcher])

  const fetchData = useCallback(() => {
    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId
    const cached = readCached()
    setState({
      data: cached,
      loading: cached === null,
      error: null,
    })
    void fetcherRef.current().then(
      (data) => {
        if (requestIdRef.current !== requestId) {
          return
        }
        setState({ data, loading: false, error: null })
      },
      (err: unknown) => {
        if (requestIdRef.current !== requestId) {
          return
        }
        const message = describeError(err, 'core_intelligence_fetch')
        setState((prev) =>
          prev.data === null
            ? { ...prev, loading: false, error: message }
            : { ...prev, loading: false, error: null },
        )
      },
    )
  }, [readCached])

  useEffect(() => {
    if (depsRef.current !== depsJson) {
      let cancelled = false
      queueMicrotask(() => {
        if (!cancelled) {
          depsRef.current = depsJson
          fetchData()
        }
      })
      return () => {
        cancelled = true
      }
    }
  }, [depsJson, fetchData, readCached])

  return { ...state, refresh: fetchData }
}

// ---------------------------------------------------------------------------
// Time range state hook
// ---------------------------------------------------------------------------

interface TimeRangeState {
  preset: TimeRangePreset
  dateRange: DateRange
  setPreset: (preset: TimeRangePreset) => void
  setCustomRange: (range: DateRange) => void
}

/**
 * Manages the global time range selection for Intelligence pages.
 *
 * Defaults to 'month' preset.
 */
export function useTimeRange(
  initialPreset: TimeRangePreset = 'month',
): TimeRangeState {
  const [preset, setPresetState] = useState<TimeRangePreset>(initialPreset)
  const [dateRange, setDateRange] = useState<DateRange>(
    dateRangeFromPreset(initialPreset),
  )

  const setPreset = useCallback((p: TimeRangePreset) => {
    setPresetState(p)
    if (p !== 'custom') {
      setDateRange(dateRangeFromPreset(p))
    }
    // Stryker disable next-line ArrayDeclaration: any constant dependency array preserves this callback contract.
  }, [])

  const setCustomRange = useCallback((range: DateRange) => {
    setPresetState('custom')
    setDateRange(range)
    // Stryker disable next-line ArrayDeclaration: any constant dependency array preserves this callback contract.
  }, [])

  return { preset, dateRange, setPreset, setCustomRange }
}
