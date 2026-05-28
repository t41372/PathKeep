/**
 * Local-time ISO date helpers shared by the paper Browse calendar, day nav,
 * and year rail.
 *
 * ## Responsibilities
 * - Convert between Date and `YYYY-MM-DD` (local time, no UTC drift — the
 *   user's archive is anchored to their local clock).
 * - Step forward/backward by N days returning ISO strings.
 * - Format an ISO date as a human-readable label in a given locale.
 * - Compute density tiers (0–4) for a day's visit count and for an aggregate
 *   year/month/period count, matching the handoff's heatmap palette breaks.
 *
 * ## Not responsible for
 * - Data fetching; callers pass counts in.
 * - Time formatting (delegated to `formatHourMinute` in group-entries.ts).
 */

export function isoFromDate(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

export function dateFromIso(iso: string): Date {
  const parts = iso.split('-').map((part) => Number.parseInt(part, 10))
  if (parts.length !== 3 || parts.some((value) => Number.isNaN(value))) {
    return new Date(NaN)
  }
  const [year, month, day] = parts
  return new Date(year, month - 1, day)
}

export function addDaysIso(iso: string, days: number): string {
  const date = dateFromIso(iso)
  if (Number.isNaN(date.getTime())) return iso
  date.setDate(date.getDate() + days)
  return isoFromDate(date)
}

export interface PrettyDayOptions {
  language?: string
  short?: boolean
  withYear?: boolean
}

export function prettyDay(iso: string, options: PrettyDayOptions = {}): string {
  const { language = 'en', short = false, withYear = true } = options
  const date = dateFromIso(iso)
  if (Number.isNaN(date.getTime())) return iso
  try {
    return date.toLocaleDateString(language, {
      weekday: short ? 'short' : 'long',
      month: 'short',
      day: 'numeric',
      year: withYear ? 'numeric' : undefined,
    })
  } catch {
    return iso
  }
}

/**
 * Maps a per-day visit count to a 5-stop density tier. The thresholds match
 * `dayTier()` in `pk-browse-nav.jsx`.
 */
export function dayDensityTier(count: number): 0 | 1 | 2 | 3 | 4 {
  if (!count || count <= 0) return 0
  if (count < 30) return 1
  if (count < 150) return 2
  if (count < 500) return 3
  return 4
}

/**
 * Maps an aggregate (year, month, period) visit count to a 5-stop tier.
 * Thresholds match `yearTier()` in `pk-browse-nav.jsx`.
 */
export function periodDensityTier(count: number): 0 | 1 | 2 | 3 | 4 {
  if (!count || count <= 0) return 0
  if (count < 5000) return 1
  if (count < 30000) return 2
  if (count < 90000) return 3
  return 4
}

/**
 * Format a relative-time hint ("today", "yesterday", "3d ago", "5w ago",
 * "1.2y ago") for the day-nav pill. `referenceIso` defaults to the latest
 * archived day, so callers pass it explicitly to avoid Date.now in render.
 */
export function relativeDayLabel(
  iso: string,
  referenceIso: string,
  copy: RelativeDayCopy,
): string {
  const target = dateFromIso(iso).getTime()
  const reference = dateFromIso(referenceIso).getTime()
  if (Number.isNaN(target) || Number.isNaN(reference)) return ''
  const days = Math.round((reference - target) / 86_400_000)
  if (days === 0) return copy.today
  if (days === 1) return copy.yesterday
  if (days < 7) return copy.daysAgo.replace('{count}', String(days))
  if (days < 60)
    return copy.weeksAgo.replace('{count}', String(Math.round(days / 7)))
  if (days < 730)
    return copy.monthsAgo.replace('{count}', String(Math.round(days / 30)))
  return copy.yearsAgo.replace('{count}', (days / 365).toFixed(1))
}

export interface RelativeDayCopy {
  today: string
  yesterday: string
  /** Template: "{count}d ago" */
  daysAgo: string
  /** Template: "{count}w ago" */
  weeksAgo: string
  /** Template: "{count}mo ago" */
  monthsAgo: string
  /** Template: "{count}y ago" */
  yearsAgo: string
}
