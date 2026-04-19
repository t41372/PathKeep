/**
 * Non-React helpers shared by Intelligence section modules.
 *
 * Why this file exists:
 * - Fast Refresh lint rules require non-component helpers to live outside the
 *   component module.
 * - The section split still needs one tiny place for formatting and metadata
 *   helpers.
 */

import type {
  CoreIntelligenceSectionMeta,
  DateRange,
} from '../../../lib/core-intelligence'

export type T = (key: string, vars?: Record<string, string | number>) => string

export function firstSectionMeta(
  ...results: Array<{ meta: CoreIntelligenceSectionMeta } | null | undefined>
) {
  return results.find((result) => result?.meta)?.meta ?? null
}

export function singleDayRange(dateKey: string): DateRange {
  return {
    start: dateKey,
    end: dateKey,
  }
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const minutes = seconds / 60
  if (minutes < 60) return `${minutes.toFixed(1)}m`
  const hours = minutes / 60
  return `${hours.toFixed(1)}h`
}

export function formatHourRange(hour: number): string {
  const start = String(hour).padStart(2, '0')
  const end = String((hour + 1) % 24).padStart(2, '0')
  return `${start}:00-${end}:00`
}

export function formatIsoDate(value: string): string {
  return value.slice(0, 10)
}

export function formatNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}
