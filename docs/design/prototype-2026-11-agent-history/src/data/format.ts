/** zh-TW formatting for counts, tokens, durations, sizes and dates. */
import { NOW, DAY } from './fixture'

const nf = new Intl.NumberFormat('zh-TW')

export const num = (n: number) => nf.format(Math.round(n))

/** 1.25 億, 3,420 萬, 8,120. Chinese readers count in 萬 and 億. */
export function big(n: number): string {
  const abs = Math.abs(n)
  if (abs >= 1e8) return `${trim(n / 1e8, abs >= 1e10 ? 0 : 2)} 億`
  if (abs >= 1e4) return `${trim(n / 1e4, abs >= 1e6 ? 0 : 1)} 萬`
  return num(n)
}

function trim(n: number, digits: number) {
  return nf.format(Number(n.toFixed(digits)))
}

export function duration(ms: number, short = false): string {
  const minutes = Math.round(ms / 60_000)
  if (ms < 60_000) return `${Math.max(1, Math.round(ms / 1000))} 秒`
  if (minutes < 60) return `${minutes} 分`
  const hours = Math.floor(minutes / 60)
  const rest = minutes % 60
  if (hours >= 100 || short) return `${num(Math.round(ms / 3_600_000))} 小時`
  return rest ? `${hours} 小時 ${rest} 分` : `${hours} 小時`
}

export function hours(ms: number): string {
  const h = ms / 3_600_000
  return h >= 10 ? `${num(h)} 小時` : `${nf.format(Number(h.toFixed(1)))} 小時`
}

export function bytes(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(n >= 1e10 ? 0 : 1)} GB`
  if (n >= 1e6) return `${(n / 1e6).toFixed(n >= 1e8 ? 0 : 1)} MB`
  if (n >= 1e3) return `${Math.round(n / 1e3)} KB`
  return `${n} B`
}

export function usd(n: number): string {
  return n >= 100 ? `US$${num(n)}` : `US$${n.toFixed(2)}`
}

export function pct(n: number, digits = 0): string {
  return `${(n * 100).toFixed(digits)}%`
}

const tz = 'Asia/Taipei'
const dayKey = (t: number) =>
  new Intl.DateTimeFormat('en-CA', { timeZone: tz }).format(new Date(t))

export const localDayKey = dayKey

export function time(t: number) {
  return new Intl.DateTimeFormat('zh-TW', { timeZone: tz, hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(t))
}

/** 今天 14:32 / 昨天 / 週二 / 10 月 28 日 / 2025 年 6 月 3 日. */
export function when(t: number): string {
  const days = Math.round((Date.parse(dayKey(NOW)) - Date.parse(dayKey(t))) / DAY)
  if (days === 0) return `今天 ${time(t)}`
  if (days === 1) return `昨天 ${time(t)}`
  if (days < 7) return new Intl.DateTimeFormat('zh-TW', { timeZone: tz, weekday: 'short' }).format(new Date(t)) + ` ${time(t)}`
  return date(t)
}

export function date(t: number, withYear?: boolean): string {
  const sameYear = dayKey(t).slice(0, 4) === dayKey(NOW).slice(0, 4)
  return new Intl.DateTimeFormat('zh-TW', {
    timeZone: tz,
    year: sameYear && !withYear ? undefined : 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(new Date(t))
}

export function dateTime(t: number) {
  return `${date(t, true)} ${time(t)}`
}

/** Section label for a day in a timeline. */
export function dayLabel(key: string): string {
  const days = Math.round((Date.parse(dayKey(NOW)) - Date.parse(key)) / DAY)
  const t = Date.parse(key) + 12 * 3600_000
  const d = new Intl.DateTimeFormat('zh-TW', { timeZone: 'UTC', month: 'long', day: 'numeric', weekday: 'short', year: key.slice(0, 4) === dayKey(NOW).slice(0, 4) ? undefined : 'numeric' }).format(new Date(t))
  if (days === 0) return `今天 · ${d}`
  if (days === 1) return `昨天 · ${d}`
  return d
}

export function ago(t: number): string {
  const days = Math.floor((NOW - t) / DAY)
  if (days < 1) return '今天'
  if (days < 30) return `${days} 天前`
  if (days < 365) return `${Math.floor(days / 30)} 個月前`
  return `${(days / 365).toFixed(1)} 年前`
}
