/**
 * @file year-heatmap-helpers.ts
 * @description Pure helpers that turn a sparse list of daily visit counts into the 7×N grid the YearHeatmap renders.
 * @module components/heatmap
 *
 * ## Responsibilities
 * - Build a dense day-keyed map from the API response.
 * - Walk from a `start` ISO date forward `days` cells, emitting one cell per
 *   day so the visual grid stays aligned with the calendar (gaps render as
 *   empty / level-0).
 * - Bucket non-zero visit counts into discrete levels 1-4 so the heatmap
 *   palette stays readable independent of archive scale.
 *
 * ## Not responsible for
 * - Date-range computation (lives in `dashboard-helpers.ts`).
 * - Network IO or rendering — these are pure functions.
 *
 * ## Performance notes
 * - 365 days × ~10 ops/day = a few thousand operations per render. Safe to
 *   call inside render paths without memoization, though the route memoizes
 *   anyway to keep React reconciliation cheap.
 */

export interface DailyVisitPoint {
  /** YYYY-MM-DD ISO date string */
  dateKey: string
  totalVisits: number
}

export interface YearHeatmapCell {
  /** YYYY-MM-DD ISO date string */
  date: string
  /** Raw count (zero if no rollup row existed for the day). */
  count: number
  /** Display bucket 0-4: 0 = empty, 1-4 = ramped accent fill. */
  level: 0 | 1 | 2 | 3 | 4
  /** 0-6, Sunday-first day-of-week. */
  dayOfWeek: number
}

/**
 * Returns YYYY-MM-DD for a Date in local time.
 */
export function isoDateOnly(date: Date): string {
  const year = date.getFullYear().toString().padStart(4, '0')
  const month = (date.getMonth() + 1).toString().padStart(2, '0')
  const day = date.getDate().toString().padStart(2, '0')
  return `${year}-${month}-${day}`
}

/**
 * Picks the cell level for a non-zero visit count given the day with the
 * archive's heaviest visit total. Level 0 is reserved for zero-count days so
 * the empty palette stays distinct from a sparse-but-present day.
 *
 * Bucketing is intentionally coarse (max/4 quartiles): a percentile sort
 * would scale to large archives but it also flattens out quiet weeks, which
 * makes the heatmap less honest for users with mostly-light browsing days.
 */
export function bucketLevel(
  count: number,
  max: number,
): YearHeatmapCell['level'] {
  if (count <= 0) return 0
  if (max <= 0) return 0
  const ratio = count / max
  if (ratio > 0.75) return 4
  if (ratio > 0.5) return 3
  if (ratio > 0.25) return 2
  return 1
}

/**
 * Builds the dense 7×N cell grid for the heatmap.
 *
 * @param points Sparse list of (dateKey, totalVisits) — days with zero
 *   visits typically do not appear in the input and are filled in here.
 * @param startDate Inclusive start of the window — must be the same date the
 *   caller queried so the calendar stays aligned.
 * @param days Total number of days in the window (typically 365).
 */
export function buildYearHeatmapCells(
  points: DailyVisitPoint[],
  startDate: Date,
  days: number,
): YearHeatmapCell[] {
  const byDate = new Map<string, number>()
  for (const point of points) {
    byDate.set(point.dateKey, point.totalVisits)
  }
  let max = 0
  for (const value of byDate.values()) {
    if (value > max) max = value
  }
  const cells: YearHeatmapCell[] = []
  for (let offset = 0; offset < days; offset += 1) {
    const day = new Date(startDate)
    day.setDate(startDate.getDate() + offset)
    const dateKey = isoDateOnly(day)
    const count = byDate.get(dateKey) ?? 0
    cells.push({
      date: dateKey,
      count,
      level: bucketLevel(count, max),
      dayOfWeek: day.getDay(),
    })
  }
  return cells
}

/**
 * Computes the longest streak of consecutive non-zero days that ends on or
 * before the most recent cell. Used by the heatmap header strip.
 */
export function longestRecentStreak(cells: YearHeatmapCell[]): number {
  let best = 0
  let current = 0
  for (const cell of cells) {
    if (cell.count > 0) {
      current += 1
      if (current > best) best = current
    } else {
      current = 0
    }
  }
  return best
}
