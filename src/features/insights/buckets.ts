/**
 * The bars of a range chart: every day of the range, zero where nothing
 * happened, grouped into Monday-start weeks for the one-year range.
 *
 * Every bar is keyed by the local date it starts on (`YYYY-MM-DD`), whatever
 * its length, so axes format the key as a date and a day bar can link to its
 * day. The backend's own week keys (`2025-W41`) are not dates, which is why
 * charts ask it for days and group them here.
 *
 * Responsible for: filling and grouping. Not responsible for: fetching.
 */
import { localDateKey } from '@/lib/backend-client/insights'
import { rangeDates, rangeDays, type RangeId } from './range'

export type BarLength = 'day' | 'week'

export function barLength(range: RangeId): BarLength {
  return range === 'y1' ? 'week' : 'day'
}

type Counts = Record<string, number>
export type Bar<T extends Counts> = T & { dateKey: string }

/** One bar per day (or week) of `range`; `byDay` holds the days with data. */
export function rangeBars<T extends Counts>(
  range: RangeId,
  byDay: ReadonlyMap<string, T>,
  zero: T,
): Bar<T>[] {
  const [year, month, day] = rangeDates(range).start.split('-').map(Number)
  const cursor = new Date(year, month - 1, day)
  const days = Array.from({ length: rangeDays[range] }, () => {
    const dateKey = localDateKey(cursor)
    cursor.setDate(cursor.getDate() + 1)
    return { ...zero, ...byDay.get(dateKey), dateKey }
  })
  return barLength(range) === 'week' ? toWeeks(days, zero) : days
}

function toWeeks<T extends Counts>(days: Bar<T>[], zero: T): Bar<T>[] {
  const weeks: Bar<T>[] = []
  for (const day of days) {
    const [year, month, date] = day.dateKey.split('-').map(Number)
    const monday = new Date(year, month - 1, date).getDay() === 1
    if (monday || weeks.length === 0) {
      weeks.push({ ...zero, dateKey: day.dateKey })
    }
    const week = weeks[weeks.length - 1] as Counts
    for (const field of Object.keys(zero)) week[field] += day[field]
  }
  return weeks
}
