/**
 * The time range every Insights screen is scoped to, kept in `?range=` so a
 * drill-in opens with the range the user was looking at and a reload keeps it.
 *
 * Responsible for: the range ids and reading and writing the param (the
 * toggle is `RangeToggle` in `parts.tsx`).
 * Not responsible for: fetching; queries take the range id.
 */
import { useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'
import { lastDays } from '@/lib/backend-client/insights'
import type { DateRange } from '@/lib/core-intelligence/types'

export const rangeDays = { d7: 7, d30: 30, d90: 90, y1: 365 } as const
export type RangeId = keyof typeof rangeDays
export const defaultRange: RangeId = 'd30'

export function isRangeId(value: string | null): value is RangeId {
  return value !== null && value in rangeDays
}

/** The local days a range covers, ending today. */
export function rangeDates(range: RangeId): DateRange {
  return lastDays(rangeDays[range])
}

export function useRange() {
  const [params, setParams] = useSearchParams()
  const requested = params.get('range')
  const range = isRangeId(requested) ? requested : defaultRange
  const setRange = useCallback(
    (next: RangeId) =>
      setParams(
        (previous) => {
          const updated = new URLSearchParams(previous)
          updated.set('range', next)
          return updated
        },
        { replace: true },
      ),
    [setParams],
  )
  return [range, setRange] as const
}
