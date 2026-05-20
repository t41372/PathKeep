/**
 * @file dashboard-helpers.ts
 * @description Pure helpers for the Dashboard route — date ranges, byte formatting, span formatting, and visit-count compaction.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Compute the date range the dashboard cards use to query backend data.
 * - Format raw numbers (visit counts, storage bytes, time spans) into the
 *   compact strings the paper cards render.
 *
 * ## Not responsible for
 * - Owning any React state or backend transport — these are pure helpers.
 * - Locale-specific number formatting beyond ASCII compact suffixes (callers
 *   that need ICU should use Intl directly).
 *
 * ## Dependencies
 * - None.
 *
 * ## Performance notes
 * - Each helper is O(1) or O(constant input length); safe to call inside
 *   render paths or per-row reducers.
 */

import type { DateRange } from '@/lib/core-intelligence'
import type { StorageSummary } from '@/lib/types'

/**
 * Returns the YYYY-MM-DD ISO date string for the given Date. The dashboard
 * date ranges only use the date portion, so trimming the time stops timezone
 * drift from leaking into the backend query window.
 */
export function isoDateOnly(date: Date): string {
  const year = date.getFullYear().toString().padStart(4, '0')
  const month = (date.getMonth() + 1).toString().padStart(2, '0')
  const day = date.getDate().toString().padStart(2, '0')
  return `${year}-${month}-${day}`
}

/**
 * Computes the date range the dashboard's active-threads card sends to
 * `getPathFlows`. Defaults to the last 30 calendar days ending today —
 * recent enough to feel like "what you've been thinking about" without
 * scoping so narrowly that small archives never produce signal.
 *
 * @param now Reference instant — pass `new Date()` in real callers; tests
 *   pass a fixed Date so assertions stay deterministic.
 */
export function dashboardThreadsRange(now: Date): DateRange {
  const end = new Date(now)
  end.setHours(0, 0, 0, 0)
  const start = new Date(end)
  start.setDate(start.getDate() - 29)
  return { start: isoDateOnly(start), end: isoDateOnly(end) }
}

/**
 * Computes the date range the dashboard's year heatmap renders. Always the
 * past 365 days ending today so the heatmap is a stable rolling window
 * instead of a calendar-year reset.
 */
export function dashboardHeatmapRange(now: Date): DateRange {
  const end = new Date(now)
  end.setHours(0, 0, 0, 0)
  const start = new Date(end)
  start.setDate(start.getDate() - 364)
  return { start: isoDateOnly(start), end: isoDateOnly(end) }
}

/**
 * Compact representation of a positive integer count: `1234567 → "1.2M"`,
 * `2400 → "2.4K"`, `42 → "42"`. The paper hero band uses this to keep the
 * stat strip aligned even when archives grow to millions of visits.
 */
export function compactNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}

const STORAGE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const

/**
 * Compact representation of a byte count using IEC-style 1024 steps. Returns
 * an empty string for zero/negative input so the dashboard renders an explicit
 * "0 B" placeholder upstream when storage has not been measured yet.
 */
export function humanizeBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return ''
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < STORAGE_UNITS.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${STORAGE_UNITS[unit]}`
}

/**
 * Sums all archive storage categories into a single byte total used by the
 * archive card and the hero stat strip.
 */
export function sumStorageBytes(storage: StorageSummary): number {
  return (
    storage.archiveDatabaseBytes +
    storage.sourceEvidenceDatabaseBytes +
    storage.searchDatabaseBytes +
    storage.intelligenceDatabaseBytes +
    storage.manifestBytes +
    storage.snapshotBytes +
    storage.exportBytes +
    storage.stagingBytes +
    storage.quarantineBytes +
    storage.semanticSidecarBytes +
    storage.intelligenceBlobBytes
  )
}

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Formats an archive span — the elapsed time between the last successful
 * backup and `now` — as the compact `2y 4m` / `5m` / `12d` / today label the
 * hero strip uses. Returns the em-dash placeholder on bad input rather than
 * throwing, so a corrupt ISO timestamp does not blank the hero strip.
 */
export function formatSpan(
  isoTimestamp: string,
  t: Translator,
  now: Date = new Date(),
): string {
  try {
    const last = new Date(isoTimestamp)
    if (Number.isNaN(last.getTime())) return '—'
    const diffMs = now.getTime() - last.getTime()
    const years = diffMs / (365.25 * 24 * 60 * 60 * 1000)
    const months = diffMs / (30 * 24 * 60 * 60 * 1000)
    const days = diffMs / (24 * 60 * 60 * 1000)
    if (years >= 1) {
      const wholeYears = Math.floor(years)
      const remMonths = Math.floor((years - wholeYears) * 12)
      return t('dashboard.spanYearsAndMonths', {
        years: wholeYears,
        months: remMonths,
      })
    }
    if (months >= 1) {
      return t('dashboard.spanMonths', { months: Math.floor(months) })
    }
    if (days >= 1) {
      return t('dashboard.spanDays', { days: Math.floor(days) })
    }
    return t('dashboard.spanToday')
  } catch {
    return '—'
  }
}
