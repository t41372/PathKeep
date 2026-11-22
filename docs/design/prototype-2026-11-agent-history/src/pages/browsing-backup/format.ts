/**
 * zh-TW formatting as the app's `useFormat()` (`src/lib/i18n/format.ts`)
 * writes it on the Backup page: numbers, dates (`dateStyle: medium`), "今天
 * 下午2:12", relative times and 1024-based sizes. The prototype's shared
 * `~/data/format` writes these differently, so this page keeps its own copy.
 */
import { DAY, NOW } from '~/data/fixture'

const tz = 'Asia/Taipei'
const numberFormat = new Intl.NumberFormat('zh-TW')
const clock = new Intl.DateTimeFormat('zh-TW', { timeZone: tz, hour: 'numeric', minute: '2-digit', hourCycle: 'h12' })
const medium = new Intl.DateTimeFormat('zh-TW', { timeZone: tz, dateStyle: 'medium' })
const monthDay = new Intl.DateTimeFormat('zh-TW', { timeZone: tz, month: 'short', day: 'numeric' })
const dayKey = (t: number) => new Intl.DateTimeFormat('en-CA', { timeZone: tz }).format(new Date(t))

export const number = (value: number) => numberFormat.format(value)

/** 「下午2:12」: zh-TW's own clock is 12-hour. */
export const time = (t: number) => clock.format(new Date(t))

/** 「2025年6月3日」. */
export const date = (t: number) => medium.format(new Date(t))

/** 「今天 下午2:12」, 「昨天 上午9:41」, otherwise 「10月28日 下午2:12」. */
export function dayAndTime(t: number) {
  const days = Math.round((Date.parse(dayKey(NOW)) - Date.parse(dayKey(t))) / DAY)
  if (days === 0) return `今天 ${time(t)}`
  if (days === 1) return `昨天 ${time(t)}`
  return `${monthDay.format(new Date(t))} ${time(t)}`
}

/** 「剛剛」, 「6 分鐘前」, 「3 小時前」, 「9 天前」, then the date. */
export function relative(t: number) {
  const minutes = Math.floor(Math.max(0, NOW - t) / 60_000)
  if (minutes < 1) return '剛剛'
  if (minutes < 60) return `${minutes} 分鐘前`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小時前`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days} 天前`
  return date(t)
}

export function bytes(value: number) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = value
  let unit = 0
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024
    unit += 1
  }
  const digits = size >= 100 || unit === 0 ? 0 : size >= 10 ? 1 : 2
  return `${new Intl.NumberFormat('zh-TW', { maximumFractionDigits: digits }).format(size)} ${units[unit]}`
}

export const baseName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path

/** `run-duration.ts`: how long a run took, or null while it runs. */
export function runDuration(run: { startedAt: number; finishedAt?: number | null }) {
  if (!run.finishedAt) return null
  const seconds = (run.finishedAt - run.startedAt) / 1000
  if (seconds < 60) return `${seconds.toFixed(1)} 秒`
  return `${Math.floor(seconds / 60)} 分 ${Math.round(seconds % 60)} 秒`
}
