/**
 * The bars of a range chart: every day of the range, zero where nothing
 * happened, grouped into Monday-start weeks or calendar months when the range
 * is long. All time starts at the first day with data.
 *
 * Every bar is keyed by the local date it starts on (`YYYY-MM-DD`), whatever
 * its length, so axes format the key as a date and a day bar can link to its
 * day. The backend's own week keys (`2025-W41`) are not dates, which is why
 * charts ask it for days and group them here.
 *
 * Responsible for: filling and grouping. Not responsible for: fetching.
 */
import { localDateKey } from '@/lib/backend-client/insights'
import type { DateRange } from '@/lib/core-intelligence/types'
import type { Formatters } from '@/lib/i18n'
import type { Scope } from './range'

export type BarLength = 'day' | 'week' | 'month'

/** Days up to about three months, weeks up to about eighteen, then months. */
export function barLength(days: number): BarLength {
  if (days <= 92) return 'day'
  if (days <= 548) return 'week'
  return 'month'
}

type Counts = Record<string, number>
export type Bar<T extends Counts> = T & { dateKey: string }

export interface Bars<T extends Counts> {
  length: BarLength
  bars: Bar<T>[]
}

const parse = (key: string) => new Date(`${key}T00:00:00`)

/** The days the chart covers: the scope's, or from the first data for all time. */
export function chartDates(scope: Scope, dataDays: Iterable<string>) {
  if (scope.range !== 'all') return scope.dates
  let first = scope.dates.end
  for (const day of dataDays) if (day < first) first = day
  return { start: first, end: scope.dates.end }
}

/** One bar per day, week or month of `dates`; `byDay` holds the days with data. */
export function rangeBars<T extends Counts>(
  dates: DateRange,
  byDay: ReadonlyMap<string, T>,
  zero: T,
): Bars<T> {
  const days: Bar<T>[] = []
  const end = parse(dates.end)
  for (
    const cursor = parse(dates.start);
    cursor <= end;
    cursor.setDate(cursor.getDate() + 1)
  ) {
    const dateKey = localDateKey(cursor)
    days.push({ ...zero, ...byDay.get(dateKey), dateKey })
  }
  const length = barLength(days.length)
  if (length === 'day') return { length, bars: days }
  const startsBar =
    length === 'week'
      ? (date: Date) => date.getDay() === 1
      : (date: Date) => date.getDate() === 1
  const bars: Bar<T>[] = []
  for (const day of days) {
    if (bars.length === 0 || startsBar(parse(day.dateKey))) {
      bars.push({ ...zero, dateKey: day.dateKey })
    }
    const bar = bars[bars.length - 1] as Counts
    for (const field of Object.keys(zero)) bar[field] += day[field]
  }
  return { length, bars }
}

/** An axis label for a bar starting on `dateKey`. */
export function barLabel(
  format: Formatters,
  length: BarLength,
  dateKey: string,
) {
  return length === 'month'
    ? format.date(parse(dateKey), { month: 'short', year: 'numeric' })
    : format.monthDay(parse(dateKey))
}
