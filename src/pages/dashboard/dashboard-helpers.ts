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

import type {
  DateRange,
  DiscoveryTrendPoint,
  PathFlow,
} from '@/lib/core-intelligence'
import type { BackupRunOverview, StorageSummary } from '@/lib/types'

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
 * Returns the first registrable domain among a path flow's steps, or `null`
 * when no step carries one (e.g. every step is a search-query group rather
 * than a site).
 *
 * The Active Threads card deep-links a clicked flow into the domain deep-dive,
 * which is the only Intelligence route that actually surfaces a focused
 * path-flow (it renders the `pathFlowFocusBody` callout when the focused flow
 * touches the page's domain). That route is addressed by a registrable domain,
 * so the dashboard must pick one from the flow itself — landing on a step's own
 * domain guarantees the focus callout matches. A flow with no domain step has
 * no such destination, so callers fall back to the overview deep-link.
 */
export function firstRegistrableDomainStep(flow: PathFlow): string | null {
  for (const step of flow.steps) {
    if (step.registrableDomain) return step.registrableDomain
  }
  return null
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
 * Computes the current ISO-week window (Monday → Sunday) that the "This week"
 * card queries. The card's badge shows the ISO week number, so the stats below
 * it must measure exactly that calendar week — otherwise the heading lies about
 * what the numbers represent. ISO weeks start on Monday, so the start is the
 * Monday on/before `now` and the end is the following Sunday.
 *
 * @param now Reference instant — pass `new Date()` in real callers; tests pass
 *   a fixed Date so the window boundary stays deterministic.
 */
export function dashboardWeekRange(now: Date): DateRange {
  const start = new Date(now)
  start.setHours(0, 0, 0, 0)
  // `getDay()` is 0 (Sun)..6 (Sat); map Sunday to 7 so Monday is offset 0.
  const isoWeekday = start.getDay() || 7
  start.setDate(start.getDate() - (isoWeekday - 1))
  const end = new Date(start)
  end.setDate(end.getDate() + 6)
  return { start: isoDateOnly(start), end: isoDateOnly(end) }
}

/**
 * Aggregated weekly counters the "This week" card renders. Sourced from the
 * deterministic discovery-trend points so the numbers reflect actual browsing
 * in the week rather than archive-wide totals.
 */
export interface WeekTrendTotals {
  /** Total page visits recorded across the week's daily points. */
  totalVisits: number
  /** Newly discovered registrable domains across the week's daily points. */
  newDomains: number
}

/**
 * Sums the per-day discovery-trend points into the week's visit and
 * new-domain totals. Pure reducer so the card stays a thin render shell and
 * the math is unit-testable in isolation.
 *
 * Tolerates `null`/`undefined` (the API's optional `data.points`) by treating
 * it as an empty week, which keeps the zero-state honest instead of throwing.
 */
export function sumWeekTrend(
  points: readonly DiscoveryTrendPoint[] | null | undefined,
): WeekTrendTotals {
  let totalVisits = 0
  let newDomains = 0
  for (const point of points ?? []) {
    totalVisits += point.totalVisits
    newDomains += point.newDomainCount
  }
  return { totalVisits, newDomains }
}

/**
 * Counts how many backup runs started inside the inclusive `[start, end]`
 * local-date window. The dashboard snapshot already carries the recent runs,
 * so the "This week" Runs stat filters them locally instead of issuing another
 * backend query. Runs with a missing/malformed `startedAt` are skipped rather
 * than counted, so a corrupt timestamp never inflates the weekly tally.
 *
 * @param range Inclusive YYYY-MM-DD window — pass `dashboardWeekRange(now)`.
 */
export function countRunsInRange(
  runs: readonly BackupRunOverview[],
  range: DateRange,
): number {
  let count = 0
  for (const run of runs) {
    if (!run.startedAt) continue
    const started = new Date(run.startedAt)
    if (Number.isNaN(started.getTime())) continue
    const dayKey = isoDateOnly(started)
    if (dayKey >= range.start && dayKey <= range.end) count += 1
  }
  return count
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
 * Formats an archive coverage span — the elapsed time between the earliest
 * and latest visit in the archive — as the compact `2y 4m` / `5m` / `12d` /
 * today label the hero strip uses. Pre-import users see "today" because every
 * visit landed within the same calendar day; users with imported takeout data
 * see the real range. Returns the em-dash placeholder when either bound is
 * missing or malformed, so corrupt ISO timestamps do not blank the hero strip.
 *
 * Pass the latest bound as `endIso` (defaults to `now`) so a stale archive
 * still measures its own coverage instead of the wall-clock gap to "now".
 */
export function formatSpan(
  startIso: string | null | undefined,
  t: Translator,
  endIso: string | null | undefined = null,
  now: Date = new Date(),
): string {
  if (!startIso) return '—'
  try {
    const start = new Date(startIso)
    if (Number.isNaN(start.getTime())) return '—'
    const end = endIso ? new Date(endIso) : now
    if (Number.isNaN(end.getTime())) return '—'
    const diffMs = end.getTime() - start.getTime()
    if (diffMs < 0) return t('dashboard.spanToday')
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
