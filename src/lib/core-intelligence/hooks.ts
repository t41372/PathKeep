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
import type { DateRange, TimeRangePreset } from './types'

// ---------------------------------------------------------------------------
// Time range helpers
// ---------------------------------------------------------------------------

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
    case 'custom':
      // Custom requires explicit start/end — default to month as fallback
      return dateRangeFromPreset('month')
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

interface AsyncState<T> {
  data: T | null
  loading: boolean
  error: string | null
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
): AsyncState<T> & { refresh: () => void } {
  const [state, setState] = useState<AsyncState<T>>({
    data: null,
    loading: true,
    error: null,
  })

  const depsRef = useRef<string>('')
  const requestIdRef = useRef(0)
  const depsJson = JSON.stringify(deps)

  const fetchData = useCallback(() => {
    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId
    setState((prev) => ({ ...prev, loading: true, error: null }))
    void fetcher().then(
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
        const message = err instanceof Error ? err.message : String(err)
        setState((prev) => ({ ...prev, loading: false, error: message }))
      },
    )
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [depsJson])

  useEffect(() => {
    if (depsRef.current !== depsJson) {
      depsRef.current = depsJson
      fetchData()
    }
  }, [depsJson, fetchData])

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
  }, [])

  const setCustomRange = useCallback((range: DateRange) => {
    setPresetState('custom')
    setDateRange(range)
  }, [])

  return { preset, dateRange, setPreset, setCustomRange }
}
