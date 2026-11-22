/**
 * Sizes and dates the way the app's `useFormat` writes them in zh-TW
 * (`src/lib/i18n/format.ts`): 1024-based sizes with 0, 1 or 2 decimals,
 * dates through `Intl` with the same options the settings dialogs pass.
 */
import { NOW } from '~/data/fixture'

const numbers = new Map<number, Intl.NumberFormat>()

function numberFormat(digits: number) {
  let format = numbers.get(digits)
  if (!format) {
    format = new Intl.NumberFormat('zh-TW', { maximumFractionDigits: digits })
    numbers.set(digits, format)
  }
  return format
}

/** `format.bytes`: 1.29 GB, 860 MB, 0 B. */
export function size(value: number) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let amount = value
  let unit = 0
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024
    unit += 1
  }
  const digits = amount >= 100 || unit === 0 ? 0 : amount >= 10 ? 1 : 2
  return `${numberFormat(digits).format(amount)} ${units[unit]}`
}

/** `format.number`. */
export const count = (value: number) => numberFormat(0).format(value)

const mediumShort = new Intl.DateTimeFormat('zh-TW', { dateStyle: 'medium', timeStyle: 'short' })

/** `format.date(value, { dateStyle: 'medium', timeStyle: 'short' })`. */
export const dateTime = (ms: number) => mediumShort.format(new Date(ms))

const clock = new Intl.DateTimeFormat('zh-TW', { timeStyle: 'short' })
const monthDay = new Intl.DateTimeFormat('zh-TW', { month: 'short', day: 'numeric' })

/** `format.dayAndTime`: 今天 / 昨天 / a short date, then the time. */
export function dayAndTime(ms: number, now = NOW) {
  const day = (t: number) => new Date(t).toDateString()
  const text = clock.format(new Date(ms))
  if (day(ms) === day(now)) return `今天 ${text}`
  if (day(ms) === day(now - 86_400_000)) return `昨天 ${text}`
  return `${monthDay.format(new Date(ms))} ${text}`
}
