/**
 * Shared route and local-date helpers for Core Intelligence entity surfaces.
 *
 * Why this file exists:
 * - Domain and day insights are first-class entities, so their route grammar
 *   should not be reassembled ad hoc inside individual pages.
 * - Local-calendar-day handling needs one reusable implementation because the
 *   product contract is based on local dates rather than raw UTC slices.
 */

import type { DateRange, TimeRangePreset } from './types'

export type InsightRouteFocusType = 'compare-set' | 'path-flow'

export interface InsightRouteFocus {
  focusType: InsightRouteFocusType
  focusId: string
}

export interface BuildIntelligenceSearchParamsOptions {
  dateRange: DateRange
  preset: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}

export function buildIntelligenceSearchParams({
  dateRange,
  preset,
  profileId,
  focus,
}: BuildIntelligenceSearchParamsOptions) {
  const params = new URLSearchParams()
  params.set('range', preset)
  if (preset === 'custom') {
    params.set('start', dateRange.start)
    params.set('end', dateRange.end)
  }
  if (profileId) {
    params.set('profileId', profileId)
  }
  if (focus?.focusId) {
    params.set('focusType', focus.focusType)
    params.set('focusId', focus.focusId)
  }
  return params
}

export function buildDayInsightsSearchParams(
  profileId?: string | null,
  focus?: InsightRouteFocus | null,
) {
  const params = new URLSearchParams()
  if (profileId) {
    params.set('profileId', profileId)
  }
  if (focus?.focusId) {
    params.set('focusType', focus.focusType)
    params.set('focusId', focus.focusId)
  }
  return params
}

export function parseInsightRouteFocus(
  searchParams: URLSearchParams,
): InsightRouteFocus | null {
  const focusType = searchParams.get('focusType')
  const focusId = searchParams.get('focusId')
  if (!focusId || (focusType !== 'compare-set' && focusType !== 'path-flow')) {
    return null
  }
  return { focusType, focusId }
}

export function isLocalDateKey(value: string) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return false
  }

  const [year, month, day] = value.split('-').map(Number)
  const date = new Date(year, month - 1, day)
  return (
    date.getFullYear() === year &&
    date.getMonth() === month - 1 &&
    date.getDate() === day
  )
}

export function formatLocalDateKey(date: Date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

export function localDateKeyFromIso(value: string) {
  return formatLocalDateKey(new Date(value))
}

export function singleDayDateRange(dateKey: string): DateRange {
  return {
    start: dateKey,
    end: dateKey,
  }
}
