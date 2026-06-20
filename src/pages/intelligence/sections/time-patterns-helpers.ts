/**
 * Pure aggregation helpers for the Intelligence hub Time-axis strips.
 *
 * ## Responsibilities
 * - Derive weekday vs weekend totals and the peak 3-hour window from browsing
 *   rhythm heatmap cells.
 *
 * ## Not responsible for
 * - Rendering (the strips live in `time-patterns.tsx`).
 * - Fetching the rhythm heatmap.
 *
 * ## Dependencies
 * - `RhythmHeatmapCell` shape from `lib/core-intelligence`.
 *
 * ## Performance notes
 * - Runs over at most 7*24 = 168 cells; negligible on the target machine. Kept in
 *   a `.ts` module so the component file only exports components (Fast Refresh).
 */

import type { RhythmHeatmapCell } from '../../../lib/core-intelligence'

export interface WeekdayWeekendSplit {
  weekdayVisits: number
  weekendVisits: number
}

/**
 * Splits heatmap cells into weekday (Mon-Fri, dow 1-5) and weekend (Sat-Sun,
 * dow 0 + 6) totals.
 */
export function computeWeekdayWeekend(
  cells: RhythmHeatmapCell[],
): WeekdayWeekendSplit {
  let weekdayVisits = 0
  let weekendVisits = 0

  for (const cell of cells) {
    if (cell.dow === 0 || cell.dow === 6) {
      weekendVisits += cell.visitCount
    } else {
      weekdayVisits += cell.visitCount
    }
  }

  return { weekdayVisits, weekendVisits }
}

export interface PeakWindow {
  /** Starting hour of the 3-hour window (0-21) */
  startHour: number
  /** Total visits in the 3-hour window */
  totalVisits: number
}

/**
 * Finds the peak 3-hour window from hourly distribution data by summing across
 * all days of week for each consecutive 3-hour block.
 */
export function computePeakHours(
  cells: RhythmHeatmapCell[],
): PeakWindow | null {
  if (cells.length === 0) return null

  // Sum visits by hour across all days.
  const hourlyTotals = new Array<number>(24).fill(0)
  for (const cell of cells) {
    hourlyTotals[cell.hour] += cell.visitCount
  }

  // Scans 0–21; a peak spanning midnight (e.g. 23:00–02:00) is split across two windows.
  let bestStart = 0
  let bestSum = 0

  for (let start = 0; start <= 21; start++) {
    const sum =
      hourlyTotals[start] + hourlyTotals[start + 1] + hourlyTotals[start + 2]
    if (sum > bestSum) {
      bestSum = sum
      bestStart = start
    }
  }

  if (bestSum === 0) return null

  return { startHour: bestStart, totalVisits: bestSum }
}
