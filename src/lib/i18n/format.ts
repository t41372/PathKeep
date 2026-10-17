/**
 * Locale-aware number and date formatting for the current language.
 * Formatter instances are cached per locale; creating Intl objects in render
 * paths is surprisingly expensive.
 *
 * Instants (a visit's time) are shown in the computer's zone, or in another
 * zone given to `createFormatters` (Insights' own, `insights/zone.ts`).
 * Calendar days (`YYYY-MM-DD` keys) go through `day`, which no zone moves.
 */
import { useMemo } from 'react'
import { dateKeyIn, dayAsUtcDate, daysBetween } from '@/lib/time-zone'
import { useI18n } from './context'
import type { Translator } from './runtime'

const cache = new Map<string, Intl.NumberFormat | Intl.DateTimeFormat>()

function numberFormat(locale: string, options: Intl.NumberFormatOptions = {}) {
  const key = `n|${locale}|${JSON.stringify(options)}`
  let format = cache.get(key) as Intl.NumberFormat | undefined
  if (!format) {
    format = new Intl.NumberFormat(locale, options)
    cache.set(key, format)
  }
  return format
}

function dateFormat(locale: string, options: Intl.DateTimeFormatOptions) {
  const key = `d|${locale}|${JSON.stringify(options)}`
  let format = cache.get(key) as Intl.DateTimeFormat | undefined
  if (!format) {
    format = new Intl.DateTimeFormat(locale, options)
    cache.set(key, format)
  }
  return format
}

type DateInput = Date | string | number

function toDate(value: DateInput) {
  return value instanceof Date ? value : new Date(value)
}

export function startOfDay(value: DateInput) {
  const date = toDate(value)
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

/** A 12- or 24-hour clock; undefined leaves it to the language. */
export type HourCycle = 'h12' | 'h23' | undefined

/** The clock a language uses when neither the user nor the OS said. */
function languageClock(locale: string): 'h12' | 'h23' {
  const cycle = dateFormat(locale, { hour: 'numeric' }).resolvedOptions()
    .hourCycle
  return cycle === 'h11' || cycle === 'h12' ? 'h12' : 'h23'
}

function clockOptions(hourCycle: 'h12' | 'h23'): Intl.DateTimeFormatOptions {
  // "6:37 AM" but "06:37": a 12-hour clock without a leading zero.
  return {
    hour: hourCycle === 'h12' ? 'numeric' : '2-digit',
    minute: '2-digit',
    hourCycle,
  }
}

/** Where instants are shown, when not in the computer's zone. */
export interface FormatZone {
  /** An IANA zone. */
  timeZone: string
  /** Times carry the zone ("14:12 GMT+8"), because it is not the computer's. */
  label: boolean
}

export function createFormatters(
  locale: string,
  t: Translator,
  hourCycle: HourCycle = undefined,
  zone: FormatZone | null = null,
) {
  const cycle = hourCycle ?? languageClock(locale)
  const timeZone = zone?.timeZone
  const clock: Intl.DateTimeFormatOptions = {
    ...clockOptions(cycle),
    timeZone,
    timeZoneName: zone?.label ? 'short' : undefined,
  }
  /** An instant formatted in the zone. */
  const at = (options: Intl.DateTimeFormatOptions) =>
    dateFormat(locale, { ...options, timeZone })
  const time = (value: DateInput) => at(clock).format(toDate(value))
  /**
   * Two times of one day as one range, the way the language writes it:
   * "7:02 – 7:23 PM", 「晚上7:02至7:23」, not the day period twice. The same
   * time twice is one time.
   */
  const timeRange = (start: DateInput, end: DateInput) => {
    const from = toDate(start)
    const to = toDate(end)
    const format = at(clock)
    return format.format(from) === format.format(to)
      ? format.format(from)
      : format.formatRange(from, to)
  }
  /** A wall-clock hour of no particular day, the same in every zone. */
  const wallClock = (value: number, options: Intl.DateTimeFormatOptions) =>
    dateFormat(locale, { ...options, timeZone: 'UTC' }).format(
      Date.UTC(2000, 0, 1, value),
    )
  /** An hour on its own, "6 AM" / "06", for chart axes. */
  const hour = (value: number) =>
    wallClock(value, {
      hour: cycle === 'h12' ? 'numeric' : '2-digit',
      hourCycle: cycle,
    })
  const clockTime = (value: number) => wallClock(value, clockOptions(cycle))
  /** Days from the instant's day to today's, in the zone. */
  const daysAgo = (value: DateInput, now: Date) => {
    if (!timeZone) {
      return Math.round(
        (startOfDay(now).getTime() - startOfDay(value).getTime()) / 86_400_000,
      )
    }
    return daysBetween(
      dateKeyIn(toDate(value).getTime(), timeZone),
      dateKeyIn(now.getTime(), timeZone),
    )
  }
  return {
    number: (value: number) => numberFormat(locale).format(value),
    compact: (value: number) =>
      numberFormat(locale, {
        notation: 'compact',
        maximumFractionDigits: 1,
      }).format(value),
    percent: (value: number, signed = false) =>
      numberFormat(locale, {
        style: 'percent',
        maximumFractionDigits: 1,
        signDisplay: signed ? 'exceptZero' : 'auto',
      }).format(value),
    bytes: (value: number) => {
      const units = ['B', 'KB', 'MB', 'GB', 'TB']
      let size = value
      let unit = 0
      while (size >= 1024 && unit < units.length - 1) {
        size /= 1024
        unit += 1
      }
      const digits = size >= 100 || unit === 0 ? 0 : size >= 10 ? 1 : 2
      return `${numberFormat(locale, { maximumFractionDigits: digits }).format(size)} ${units[unit]}`
    },
    /** The date of an instant. */
    date: (
      value: DateInput,
      options: Intl.DateTimeFormatOptions = { dateStyle: 'medium' },
    ) => at(options).format(toDate(value)),
    /** A calendar day, `YYYY-MM-DD`: the same date in every zone. */
    day: (
      key: string,
      options: Intl.DateTimeFormatOptions = { dateStyle: 'medium' },
    ) =>
      dateFormat(locale, { ...options, timeZone: 'UTC' }).format(
        dayAsUtcDate(key),
      ),
    /** Month and day of an instant, e.g. "Oct 2" / "10月2日". */
    monthDay: (value: DateInput) =>
      at({ month: 'short', day: 'numeric' }).format(toDate(value)),
    time,
    timeRange,
    hour,
    /** A wall-clock hour as a time, "6:00 AM" / "06:00". */
    clockTime,
    /** One hour of the day as a span, "6:00 AM–7:00 AM" / "06:00–07:00". */
    hourSpan: (value: number) =>
      `${clockTime(value)}–${clockTime((value + 1) % 24)}`,
    weekdayDate: (value: DateInput) =>
      at({
        weekday: 'long',
        month: 'long',
        day: 'numeric',
      }).format(toDate(value)),
    /** Relative time; elapsed keeps months and years relative for related pages. */
    relative: (
      value: DateInput,
      now = Date.now(),
      style: 'date' | 'elapsed' = 'date',
    ) => {
      const diff = Math.max(0, now - toDate(value).getTime())
      const minutes = Math.floor(diff / 60_000)
      if (minutes < 1) return t('common.justNow')
      if (minutes < 60) return t('common.minutesAgo', { count: minutes })
      const hours = Math.floor(minutes / 60)
      if (hours < 24) return t('common.hoursAgo', { count: hours })
      const days = Math.floor(hours / 24)
      if (days < 30) return t('common.daysAgo', { count: days })
      if (style === 'elapsed') {
        const months = Math.floor(days / 30)
        return months < 12
          ? t('common.monthsAgo', { count: months })
          : t('common.yearsAgo', { count: Math.floor(days / 365) || 1 })
      }
      return at({ dateStyle: 'medium' }).format(toDate(value))
    },
    /** "14:12" today, "Yesterday", otherwise a short date. */
    shortWhen: (value: DateInput, now = new Date()) => {
      const date = toDate(value)
      const days = daysAgo(date, now)
      if (days === 0) return time(date)
      if (days === 1) return t('common.yesterday')
      return at({ month: 'short', day: 'numeric' }).format(date)
    },
    /** "Today 14:12", "Yesterday 09:41", otherwise a short date and time. */
    dayAndTime: (value: DateInput, now = new Date()) => {
      const date = toDate(value)
      const days = daysAgo(date, now)
      const clockText = time(date)
      if (days === 0) return `${t('common.today')} ${clockText}`
      if (days === 1) return `${t('common.yesterday')} ${clockText}`
      return `${at({ month: 'short', day: 'numeric' }).format(date)} ${clockText}`
    },
  }
}

export type Formatters = ReturnType<typeof createFormatters>

export function useFormat(): Formatters {
  return useZonedFormat(null)
}

/** Formatters that show instants in `zone` (see `FormatZone`). */
export function useZonedFormat(zone: FormatZone | null): Formatters {
  const { locale, t, hourCycle } = useI18n()
  const timeZone = zone?.timeZone
  const label = zone?.label ?? false
  return useMemo(
    () =>
      createFormatters(
        locale,
        t,
        hourCycle,
        timeZone ? { timeZone, label } : null,
      ),
    [locale, t, hourCycle, timeZone, label],
  )
}
