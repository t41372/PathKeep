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

export function createFormatters(locale: string, t: Translator) {
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
    time: (value: DateInput) =>
      dateFormat(locale, {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      }).format(toDate(value)),
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
      if (days === 0) {
        return dateFormat(locale, {
          hour: '2-digit',
          minute: '2-digit',
          hour12: false,
        }).format(date)
      }
      if (days === 1) return t('common.yesterday')
      return dateFormat(locale, { month: 'short', day: 'numeric' }).format(date)
    },
    /** "Today 14:12", "Yesterday 09:41", otherwise a short date and time. */
    dayAndTime: (value: DateInput, now = new Date()) => {
      const date = toDate(value)
      const days = Math.round(
        (startOfDay(now).getTime() - startOfDay(date).getTime()) / 86_400_000,
      )
      const time = dateFormat(locale, {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      }).format(date)
      if (days === 0) return `${t('common.today')} ${time}`
      if (days === 1) return `${t('common.yesterday')} ${time}`
      return `${dateFormat(locale, { month: 'short', day: 'numeric' }).format(date)} ${time}`
    },
  }
}

export type Formatters = ReturnType<typeof createFormatters>

export function useFormat(): Formatters {
  const { locale, t } = useI18n()
  return useMemo(() => createFormatters(locale, t), [locale, t])
}
