/**
 * Pure helpers powering PaperExplorerView.
 *
 * Split from `paper-view.tsx` so the view file can export only its React
 * component (keeping react-refresh happy) while the helpers stay unit-
 * testable on their own.
 */

import type { PaperDay } from '@/pages/explorer/paper/group-entries'
import { dateFromIso } from '@/pages/explorer/paper/date-helpers'

export function pickInitialDate(
  targetDate: string | null,
  days: PaperDay[],
  today: string,
): string {
  if (targetDate) return targetDate
  if (days.length > 0) return days[0].date
  return today
}

export function buildPerDayDensity(
  days: PaperDay[],
  overrides: ReadonlyMap<string, number> | undefined,
): Map<string, number> {
  const map = new Map<string, number>()
  for (const day of days) map.set(day.date, day.visitCount)
  if (overrides) {
    overrides.forEach((value, key) => {
      // Overrides override loaded counts so the calendar can show density for
      // days we haven't paged in yet.
      if (!map.has(key) || (map.get(key) ?? 0) < value) map.set(key, value)
    })
  }
  return map
}

export function buildPerYearDensity(
  days: PaperDay[],
  overrides: ReadonlyMap<number, number> | undefined,
): Map<number, number> {
  const map = new Map<number, number>()
  for (const day of days) {
    const year = Number.parseInt(day.date.slice(0, 4), 10)
    if (!Number.isFinite(year)) continue
    map.set(year, (map.get(year) ?? 0) + day.visitCount)
  }
  if (overrides) {
    overrides.forEach((value, year) => {
      if (!map.has(year) || (map.get(year) ?? 0) < value) map.set(year, value)
    })
  }
  return map
}

export interface InferredBounds {
  firstIso: string
  lastIso: string
  firstYear: number
  lastYear: number
  totalDays: number
}

export function inferBounds(days: PaperDay[], today: string): InferredBounds {
  if (days.length === 0) {
    const year = Number.parseInt(today.slice(0, 4), 10)
    return {
      firstIso: today,
      lastIso: today,
      firstYear: Number.isFinite(year) ? year : new Date().getFullYear(),
      lastYear: Number.isFinite(year) ? year : new Date().getFullYear(),
      totalDays: 1,
    }
  }
  const lastIso = days[0].date
  const firstIso = days[days.length - 1].date
  const firstYear = Number.parseInt(firstIso.slice(0, 4), 10)
  const lastYear = Number.parseInt(lastIso.slice(0, 4), 10)
  const totalDays =
    Math.max(
      1,
      Math.round(
        (dateFromIso(lastIso).getTime() - dateFromIso(firstIso).getTime()) /
          86_400_000,
      ),
    ) + 1
  return { firstIso, lastIso, firstYear, lastYear, totalDays }
}
