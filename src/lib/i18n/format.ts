/**
 * Locale-aware number and date formatting for the current language.
 * Formatter instances are cached per locale; creating Intl objects in render
 * paths is surprisingly expensive.
 */
import { useMemo } from 'react'
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

export function createFormatters(
  locale: string,
  t: Translator,
  hourCycle: HourCycle = undefined,
) {
  const cycle = hourCycle ?? languageClock(locale)
  const clock = clockOptions(cycle)
  const time = (value: DateInput) =>
    dateFormat(locale, clock).format(toDate(value))
  /** An hour on its own, "6 AM" / "06", for chart axes. */
  const hour = (value: number) =>
    dateFormat(locale, {
      hour: cycle === 'h12' ? 'numeric' : '2-digit',
      hourCycle: cycle,
    }).format(new Date(2000, 0, 1, value))
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
    date: (
      value: DateInput,
      options: Intl.DateTimeFormatOptions = { dateStyle: 'medium' },
    ) => dateFormat(locale, options).format(toDate(value)),
    /** Month and day, e.g. "Oct 2" / "10月2日". */
    monthDay: (value: DateInput) =>
      dateFormat(locale, { month: 'short', day: 'numeric' }).format(
        toDate(value),
      ),
    time,
    hour,
    /** One hour of the day as a span, "6:00 AM–7:00 AM" / "06:00–07:00". */
    hourSpan: (value: number) =>
      `${time(new Date(2000, 0, 1, value))}–${time(new Date(2000, 0, 1, (value + 1) % 24))}`,
    weekdayDate: (value: DateInput) =>
      dateFormat(locale, {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
      }).format(toDate(value)),
    /** "just now", "12 min ago", "3 days ago", then a plain date. */
    relative: (value: DateInput, now = Date.now()) => {
      const diff = Math.max(0, now - toDate(value).getTime())
      const minutes = Math.floor(diff / 60_000)
      if (minutes < 1) return t('common.justNow')
      if (minutes < 60) return t('common.minutesAgo', { count: minutes })
      const hours = Math.floor(minutes / 60)
      if (hours < 24) return t('common.hoursAgo', { count: hours })
      const days = Math.floor(hours / 24)
      if (days < 30) return t('common.daysAgo', { count: days })
      return dateFormat(locale, { dateStyle: 'medium' }).format(toDate(value))
    },
    /** "14:12" today, "Yesterday", otherwise a short date. */
    shortWhen: (value: DateInput, now = new Date()) => {
      const date = toDate(value)
      const days = Math.round(
        (startOfDay(now).getTime() - startOfDay(date).getTime()) / 86_400_000,
      )
      if (days === 0) return time(date)
      if (days === 1) return t('common.yesterday')
      return dateFormat(locale, { month: 'short', day: 'numeric' }).format(date)
    },
    /** "Today 14:12", "Yesterday 09:41", otherwise a short date and time. */
    dayAndTime: (value: DateInput, now = new Date()) => {
      const date = toDate(value)
      const days = Math.round(
        (startOfDay(now).getTime() - startOfDay(date).getTime()) / 86_400_000,
      )
      const at = time(date)
      if (days === 0) return `${t('common.today')} ${at}`
      if (days === 1) return `${t('common.yesterday')} ${at}`
      return `${dateFormat(locale, { month: 'short', day: 'numeric' }).format(date)} ${at}`
    },
  }
}

export type Formatters = ReturnType<typeof createFormatters>

export function useFormat(): Formatters {
  const { locale, t, hourCycle } = useI18n()
  return useMemo(
    () => createFormatters(locale, t, hourCycle),
    [locale, t, hourCycle],
  )
}
