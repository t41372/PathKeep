/**
 * The app's zh-TW formatters that the schedule card and the 背景任務 blocks
 * use, rebuilt over the prototype's fixed NOW: `format.dayAndTime`,
 * `format.relative` and `format.number` (`src/lib/i18n/format.ts`) and
 * `formatInterval` (`src/features/backup/interval.ts`).
 */
import { DAY, NOW } from '~/data/fixture'

const tz = 'Asia/Taipei'
const dayKey = (t: number) => new Intl.DateTimeFormat('en-CA', { timeZone: tz }).format(new Date(t))
const clock = (t: number) => new Intl.DateTimeFormat('zh-TW', { timeZone: tz, hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(t))

/** 「今天 14:12」「昨天 09:41」, otherwise a short date and time. */
export function dayAndTime(t: number) {
  const days = Math.round((Date.parse(dayKey(NOW)) - Date.parse(dayKey(t))) / DAY)
  if (days === 0) return `今天 ${clock(t)}`
  if (days === 1) return `昨天 ${clock(t)}`
  return `${new Intl.DateTimeFormat('zh-TW', { timeZone: tz, month: 'short', day: 'numeric' }).format(new Date(t))} ${clock(t)}`
}

/** 剛剛 / {n} 分鐘前 / {n} 小時前 / {n} 天前, then a medium date. */
export function relative(t: number) {
  const minutes = Math.floor(Math.max(0, NOW - t) / 60_000)
  if (minutes < 1) return '剛剛'
  if (minutes < 60) return `${minutes} 分鐘前`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小時前`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days} 天前`
  return new Intl.DateTimeFormat('zh-TW', { timeZone: tz, dateStyle: 'medium' }).format(new Date(t))
}

export const number = (n: number) => new Intl.NumberFormat('zh-TW').format(n)

export type IntervalUnit = 'minutes' | 'hours' | 'days'

export const unitMinutes: Record<IntervalUnit, number> = { minutes: 1, hours: 60, days: 24 * 60 }
const unitLabel: Record<IntervalUnit, string> = { minutes: '分鐘', hours: '小時', days: '天' }

export const hoursToMinutes = (hours: number) => Math.round(hours * 60)

/** The largest unit that divides the interval evenly: 90 min stays minutes, 48 h becomes 2 days. */
export function splitInterval(minutes: number): { amount: number; unit: IntervalUnit } {
  if (minutes >= unitMinutes.days && minutes % unitMinutes.days === 0) return { amount: minutes / unitMinutes.days, unit: 'days' }
  if (minutes >= unitMinutes.hours && minutes % unitMinutes.hours === 0) return { amount: minutes / unitMinutes.hours, unit: 'hours' }
  return { amount: minutes, unit: 'minutes' }
}

/** 「90 分鐘」「6 小時」「2 天」. */
export function formatInterval(hours: number) {
  const { amount, unit } = splitInterval(hoursToMinutes(hours))
  return `${amount} ${unitLabel[unit]}`
}

/** A valid interval in hours (one minute to 30 days, whole numbers), or null. */
export function intervalHours(amount: number, unit: IntervalUnit) {
  if (!Number.isInteger(amount)) return null
  const minutes = amount * unitMinutes[unit]
  if (minutes < 1 || minutes > 30 * 24 * 60) return null
  return minutes / 60
}
